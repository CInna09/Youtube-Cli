/// Application state machine + event loop.

use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event as CEvent, KeyCode, KeyEvent, KeyModifiers};

use crate::model::{Playlist, RepeatMode, Track, Video};
use crate::mpv::Mpv;
use crate::ytdlp::Ytdlp;
use crate::visualizer::Visualizer;

// ── Input mode ──

#[derive(Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
}

// ── View state ──

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Search,
}

// ── App state ──

pub struct App {
    pub input_mode: InputMode,
    pub width: u16,
    pub height: u16,
    pub view: View,

    // Search
    pub query: String,
    pub search_query: String,  // original query disimpan buat load-more
    pub results: Vec<Video>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub searching: bool,
    pub error: Option<String>,

    // Playback
    pub track: Option<Track>,
    pub playlist: Playlist,
    pub volume: u8,
    pub muted_volume: u8,
    pub eq_bars: Vec<u8>,

    // Animation
    pub tick: u64,

    // Auto-next guard (cegah re-trigger)
    autonext_cooldown: u8,

    // Repeat mode
    pub repeat: RepeatMode,

    // Visualizer
    viz_rx: Option<mpsc::Receiver<Vec<u8>>>,
    _visualizer: Option<Visualizer>,

    // Services
    ytdlp: Ytdlp,
    mpv: Mpv,

    // Async search channel
    search_rx: mpsc::Receiver<SearchResult>,
    search_tx: mpsc::Sender<SearchResult>,
}

struct SearchResult {
    videos: Vec<Video>,
    error: Option<String>,
}

impl App {
    pub fn with_config(ytdlp: Ytdlp, mpv: Mpv, default_volume: u8) -> Self {
        let (search_tx, search_rx) = mpsc::channel();
        let vol = default_volume.min(100);

        Self {
            input_mode: InputMode::Search,
            width: 80,
            height: 24,
            view: View::Search,
            query: String::new(),
            search_query: String::new(),
            results: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            searching: false,
            error: None,
            track: None,
            playlist: Playlist::new(),
            volume: vol,
            muted_volume: vol,
            eq_bars: Vec::new(),
            tick: 0,
            autonext_cooldown: 0,
            repeat: RepeatMode::Off,
            viz_rx: None,
            _visualizer: None,
            ytdlp,
            mpv,
            search_rx,
            search_tx,
        }
    }

    // ── Event loop ──

    pub fn run(&mut self) -> Result<()> {
        use ratatui::backend::CrosstermBackend;
        use ratatui::Terminal;

        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let tick_rate = Duration::from_millis(100);
        let mut last_tick = Instant::now();

        if let Err(e) = self.mpv.start() {
            self.error = Some(format!("mpv: {}", e));
        }

        // Start visualizer
        let (viz_tx, viz_rx) = mpsc::channel();
        match Visualizer::start(viz_tx) {
            Ok(viz) => {
                self._visualizer = Some(viz);
                self.viz_rx = Some(viz_rx);
            }
            Err(e) => {
                eprintln!("visualizer: {}", e);
            }
        }

        let res = self.event_loop(&mut terminal, tick_rate, &mut last_tick);

        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen
        )?;
        terminal.show_cursor()?;

        res
    }

    fn event_loop(
        &mut self,
        terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
        tick_rate: Duration,
        last_tick: &mut Instant,
    ) -> Result<()> {
        loop {
            terminal.draw(|f| crate::ui::draw(f, self))?;

            if self.searching {
                if let Ok(result) = self.search_rx.try_recv() {
                    self.searching = false;
                    self.error = result.error;
                    if result.videos.is_empty() {
                        if self.results.is_empty() {
                            self.error = Some("No results found".into());
                        } else {
                            self.error = Some("No more results".into());
                        }
                    } else if self.results.is_empty() {
                        // Fresh search — replace
                        self.results = result.videos.clone();
                        self.cursor = 0;
                        self.scroll_offset = 0;
                        self.playlist.clear();
                        self.playlist.enqueue(result.videos);
                        self.input_mode = InputMode::Normal;
                    } else {
                        // Load-more — append
                        let n = result.videos.len();
                        self.results.extend(result.videos);
                        self.playlist.enqueue(
                            self.results[self.results.len().saturating_sub(n)..].to_vec(),
                        );
                    }
                }
            }

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or(Duration::ZERO);

            if event::poll(timeout)? {
                match event::read()? {
                    CEvent::Key(key) => {
                        if self.handle_key(key)? {
                            return Ok(());
                        }
                    }
                    CEvent::Resize(w, h) => {
                        self.width = w;
                        self.height = h;
                    }
                    _ => {}
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.tick = self.tick.wrapping_add(1);

                // Update eq_bars dari visualizer
                if let Some(ref rx) = self.viz_rx {
                    let mut latest = None;
                    while let Ok(bars) = rx.try_recv() {
                        latest = Some(bars);
                    }
                    if let Some(bars) = latest {
                        self.eq_bars = bars;
                    }
                }

                if let Some(ref mut track) = self.track {
                    if let Ok(pos) = self.mpv.get_position() {
                        track.position = pos;
                    }
                    if let Ok(dur) = self.mpv.get_duration() {
                        track.duration = dur;
                    }
                    if let Ok(paused) = self.mpv.get_paused() {
                        track.paused = paused;
                    }
                    if let Ok(vol) = self.mpv.get_volume() {
                        track.volume = vol;
                        self.volume = vol;
                    }
                }

                // ── Auto-next saat lagu selesai ──
                if self.autonext_cooldown > 0 {
                    self.autonext_cooldown -= 1;
                } else if let Some(ref track) = self.track {
                    if !track.video.is_live && !track.paused {
                        let eof = self.mpv.get_eof_reached().unwrap_or(false);
                        if eof {
                            match self.repeat {
                                RepeatMode::One => {
                                    // Putar ulang lagu yang sama
                                    if let Some(video) = self.playlist.items.get(self.playlist.current).cloned() {
                                        if let Err(e) = self.play_video(video) {
                                            self.error = Some(format!("auto-repeat-one: {}", e));
                                        }
                                    }
                                }
                                RepeatMode::All => {
                                    // Loop playlist: balik ke awal
                                    self.playlist.current = 0;
                                    if let Some(video) = self.playlist.items.first().cloned() {
                                        if let Err(e) = self.play_video(video) {
                                            self.error = Some(format!("auto-repeat-all: {}", e));
                                        }
                                    }
                                }
                                RepeatMode::Off => {
                                    if self.playlist.current + 1 < self.playlist.len() {
                                        if let Err(e) = self.play_next() {
                                            self.error = Some(format!("auto-next: {}", e));
                                        }
                                    } else {
                                        self.stop().ok();
                                    }
                                }
                            }
                            self.autonext_cooldown = 10; // 1 detik cooldown
                        }
                    }
                }
                *last_tick = Instant::now();
            }
        }
    }

    // ── Keyboard handling ──

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            return Ok(true);
        }

        match self.input_mode {
            InputMode::Search => self.handle_search_mode(key),
            InputMode::Normal => self.handle_normal_mode(key),
        }
    }

    fn handle_search_mode(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::F(1) => self.toggle_mute()?,
            KeyCode::F(2) => {
                self.volume = self.volume.saturating_sub(5);
                self.mpv.set_volume(self.volume)?;
                if let Some(ref mut t) = self.track { t.volume = self.volume; }
            }
            KeyCode::F(3) => {
                self.volume = (self.volume + 5).min(100);
                self.mpv.set_volume(self.volume)?;
                if let Some(ref mut t) = self.track { t.volume = self.volume; }
            }
            KeyCode::F(4) => self.stop()?,
            KeyCode::Left => {
                if self.track.is_some() {
                    self.mpv.seek(-5.0).ok();
                }
            }
            KeyCode::Right => {
                if self.track.is_some() {
                    self.mpv.seek(5.0).ok();
                }
            }
            KeyCode::Up => {
                if !self.results.is_empty() && !self.searching {
                    self.cursor_up();
                }
            }
            KeyCode::Down => {
                if !self.results.is_empty() && !self.searching {
                    self.cursor_down();
                }
            }
            KeyCode::PageUp => {
                if !self.results.is_empty() && !self.searching {
                    self.page_up();
                }
            }
            KeyCode::PageDown => {
                if !self.results.is_empty() && !self.searching {
                    self.page_down();
                }
            }
            KeyCode::Home => {
                if !self.results.is_empty() && !self.searching {
                    self.cursor = 0;
                    self.scroll_offset = 0;
                }
            }
            KeyCode::End => {
                if !self.results.is_empty() && !self.searching {
                    self.cursor = self.results.len().saturating_sub(1);
                    self.scroll_offset = self.cursor.saturating_sub(self.visible_height());
                }
            }
            KeyCode::Enter => {
                if !self.query.is_empty() && !self.searching {
                    self.start_search();
                }
            }
            KeyCode::Esc | KeyCode::Char('/') => {
                if self.query.is_empty() {
                    self.input_mode = InputMode::Normal;
                } else {
                    self.query.clear();
                }
            }
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query.push(c);
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Search;
            }
            KeyCode::Up => {
                if !self.results.is_empty() {
                    self.cursor_up();
                }
            }
            KeyCode::Down => {
                if !self.results.is_empty() {
                    self.cursor_down();
                }
            }
            KeyCode::PageUp => {
                if !self.results.is_empty() {
                    self.page_up();
                }
            }
            KeyCode::PageDown => {
                if !self.results.is_empty() {
                    self.page_down();
                }
            }
            KeyCode::Home => {
                if !self.results.is_empty() {
                    self.cursor = 0;
                    self.scroll_offset = 0;
                }
            }
            KeyCode::End => {
                if !self.results.is_empty() {
                    self.cursor = self.results.len().saturating_sub(1);
                    self.scroll_offset = self.cursor.saturating_sub(self.visible_height());
                }
            }
            KeyCode::Enter => {
                if !self.results.is_empty() {
                    self.play_at_cursor()?;
                }
            }
            KeyCode::Char(' ') => {
                if self.track.is_some() {
                    self.mpv.toggle_pause()?;
                } else if !self.results.is_empty() {
                    self.play_at_cursor()?;
                }
            }
            // n = next
            KeyCode::Char('n') => self.play_next()?,
            // p = prev
            KeyCode::Char('p') => self.play_prev()?,
            KeyCode::Left => {
                if self.track.is_some() {
                    self.mpv.seek(-5.0).ok();
                }
            }
            KeyCode::Right => {
                if self.track.is_some() {
                    self.mpv.seek(5.0).ok();
                }
            }
            KeyCode::F(1) => self.toggle_mute()?,
            KeyCode::F(2) | KeyCode::Char('-') => {
                self.volume = self.volume.saturating_sub(5);
                self.mpv.set_volume(self.volume)?;
                if let Some(ref mut t) = self.track { t.volume = self.volume; }
            }
            KeyCode::F(3) | KeyCode::Char('=') | KeyCode::Char('+') => {
                self.volume = (self.volume + 5).min(100);
                self.mpv.set_volume(self.volume)?;
                if let Some(ref mut t) = self.track { t.volume = self.volume; }
            }
            KeyCode::F(4) | KeyCode::Char('s') => {
                self.stop()?;
            }
            // Shuffle — acak playlist
            KeyCode::Char('x') | KeyCode::Char('X') => {
                self.playlist.toggle_shuffle();
            }
            // Repeat — ganti mode
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.repeat = self.repeat.next();
            }
            // Load More — tambah hasil search
            KeyCode::Char('l') | KeyCode::Char('L') => {
                if !self.search_query.is_empty() {
                    self.load_more();
                }
            }
            _ => {}
        }
        Ok(false)
    }

    // ── Scrolling helpers ──

    /// Estimate how many rows are visible in the results area.
    /// We reserve ~5 lines for header/decorations, the rest is available.
    fn visible_height(&self) -> usize {
        self.height.saturating_sub(12).max(1) as usize
    }

    /// Move cursor up and auto-scroll if needed.
    fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            // Scroll up when cursor goes above scroll_offset
            if self.cursor < self.scroll_offset {
                self.scroll_offset = self.cursor;
            }
        }
    }

    /// Move cursor down and auto-scroll if needed.
    fn cursor_down(&mut self) {
        if self.cursor + 1 < self.results.len() {
            self.cursor += 1;
            let vis = self.visible_height().max(1);
            // Scroll down when cursor goes past the bottom of visible area
            let threshold = self.scroll_offset + vis;
            if self.cursor >= threshold {
                self.scroll_offset = self.cursor.saturating_sub(vis).min(self.results.len().saturating_sub(1));
            }
        }
    }

    /// Page up by visible_height items.
    fn page_up(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let vis = self.visible_height().max(1);
        if self.cursor > vis {
            self.cursor -= vis;
            self.scroll_offset = self.scroll_offset.saturating_sub(vis);
        } else {
            self.cursor = 0;
            self.scroll_offset = 0;
        }
    }

    /// Page down by visible_height items.
    fn page_down(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let vis = self.visible_height().max(1);
        let total = self.results.len();
        if self.cursor + vis < total {
            self.cursor += vis;
            self.scroll_offset = (self.scroll_offset + vis).min(total.saturating_sub(1));
        } else {
            self.cursor = total.saturating_sub(1);
            self.scroll_offset = self.cursor.saturating_sub(vis);
        }
    }

    // ── Async search ──

    fn start_search(&mut self) {
        let query = self.query.clone();
        if query.is_empty() {
            return;
        }

        self.searching = true;
        self.search_query = query.clone();
        self.error = None;
        self.cursor = 0;

        let tx = self.search_tx.clone();

        thread::spawn(move || {
            let ytdlp = Ytdlp::new();
            let result = match ytdlp.search(&query, 50) {
                Ok(videos) => SearchResult { videos, error: None },
                Err(e) => SearchResult { videos: Vec::new(), error: Some(e.to_string()) },
            };
            let _ = tx.send(result);
        });
    }

    fn load_more(&mut self) {
        if self.search_query.is_empty() || self.searching {
            return;
        }
        self.searching = true;
        self.error = None;

        let query = self.search_query.clone();
        let existing_ids: std::collections::HashSet<String> =
            self.results.iter().map(|v| v.id.clone()).collect();

        let tx = self.search_tx.clone();

        thread::spawn(move || {
            let ytdlp = Ytdlp::new();
            let result = match ytdlp.search(&query, 50) {
                Ok(videos) => {
                    // Filter out videos we already have (by ID)
                    let new_videos: Vec<Video> = videos
                        .into_iter()
                        .filter(|v| !existing_ids.contains(&v.id))
                        .collect();
                    SearchResult {
                        videos: new_videos,
                        error: None,
                    }
                }
                Err(e) => SearchResult {
                    videos: Vec::new(),
                    error: Some(e.to_string()),
                },
            };
            let _ = tx.send(result);
        });
    }

    // ── Playback actions ──

    fn play_at_cursor(&mut self) -> Result<()> {
        let Some(video) = self.results.get(self.cursor).cloned() else {
            return Ok(());
        };
        // sync playlist cursor ke results cursor
        self.playlist.play_at(self.cursor);
        self.play_video(video)
    }

    fn play_next(&mut self) -> Result<()> {
        let Some(video) = self.playlist.next().cloned() else {
            return Ok(());
        };
        self.cursor = self.playlist.current;
        self.play_video(video)
    }

    fn play_prev(&mut self) -> Result<()> {
        let Some(video) = self.playlist.prev().cloned() else {
            return Ok(());
        };
        self.cursor = self.playlist.current;
        self.play_video(video)
    }

    fn play_video(&mut self, video: Video) -> Result<()> {
        self.autonext_cooldown = 0; // reset guard
        let url = self.ytdlp.stream_url(&video.id)?;
        self.mpv.load_url(&url)?;
        self.mpv.set_volume(self.volume)?;

        self.track = Some(Track {
            video,
            position: 0.0,
            duration: 0.0,
            volume: self.volume,
            paused: false,
        });
        // Tetap di view Search — now-playing pindah ke sidebar
        // self.view = View::Player;

        Ok(())
    }

    fn toggle_mute(&mut self) -> Result<()> {
        let vol = self.mpv.get_volume()?;
        if vol > 0 {
            self.muted_volume = vol;
            self.volume = 0;
            self.mpv.set_volume(0)?;
        } else {
            let restore = if self.muted_volume > 0 { self.muted_volume } else { 30 };
            self.volume = restore;
            self.mpv.set_volume(restore)?;
        }
        if let Some(ref mut track) = self.track {
            track.volume = self.volume;
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.mpv.stop()?;
        self.track = None;
        self.view = View::Search;
        Ok(())
    }
}
