/// Data structures — Video, Track, Playlist.

use rand::seq::SliceRandom;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(&self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::All => "All",
            Self::One => "One",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub duration: f64,   // seconds
    pub views: u64,
    pub is_live: bool,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub video: Video,
    pub position: f64,
    pub duration: f64,
    pub volume: u8,
    pub paused: bool,
}

#[derive(Debug, Clone)]
pub struct Playlist {
    pub items: Vec<Video>,
    pub current: usize,
    pub shuffled: bool,
    original_items: Vec<Video>,
}

impl Playlist {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            current: 0,
            shuffled: false,
            original_items: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn play_at(&mut self, index: usize) -> Option<&Video> {
        if index < self.items.len() {
            self.current = index;
            self.items.get(index)
        } else {
            None
        }
    }

    /// Next item. Returns `None` if at end (caller checks repeat mode).
    pub fn next(&mut self) -> Option<&Video> {
        if self.current + 1 < self.items.len() {
            self.current += 1;
            self.items.get(self.current)
        } else {
            None
        }
    }

    pub fn prev(&mut self) -> Option<&Video> {
        if self.current > 0 {
            self.current -= 1;
            self.items.get(self.current)
        } else {
            None
        }
    }

    pub fn enqueue(&mut self, videos: Vec<Video>) {
        self.items.extend(videos);
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current = 0;
        self.shuffled = false;
        self.original_items.clear();
    }

    // ── Shuffle ──

    pub fn shuffle(&mut self) {
        if self.shuffled || self.items.is_empty() {
            return;
        }
        self.shuffled = true;
        self.original_items = self.items.clone();
        let current_id = self.items.get(self.current).map(|v| v.id.clone());
        let mut rng = rand::rng();
        self.items.shuffle(&mut rng);
        // cari posisi item yang sedang diputar di hasil shuffle
        if let Some(ref id) = current_id {
            self.current = self.items.iter().position(|v| v.id == *id).unwrap_or(0);
        }
    }

    pub fn unshuffle(&mut self) {
        if !self.shuffled || self.original_items.is_empty() {
            return;
        }
        let current_id = self.items.get(self.current).map(|v| v.id.clone());
        self.items = std::mem::take(&mut self.original_items);
        self.shuffled = false;
        if let Some(ref id) = current_id {
            self.current = self.items.iter().position(|v| v.id == *id).unwrap_or(0);
        }
    }

    pub fn toggle_shuffle(&mut self) {
        if self.shuffled {
            self.unshuffle();
        } else {
            self.shuffle();
        }
    }
}

/// Format seconds menjadi MM:SS atau H:MM:SS
pub fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    if total == 0 {
        return "--:--".into();
    }
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

/// Format angka view (1234567 → "1.2m")
pub fn format_views(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}k", n as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.1}m", n as f64 / 1_000_000.0),
        _ => format!("{:.1}b", n as f64 / 1_000_000_000.0),
    }
}
