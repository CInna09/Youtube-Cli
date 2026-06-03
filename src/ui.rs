/// ratatui rendering — adopsi referensi UI baru.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{App, InputMode};
use crate::model::{format_duration, format_views};

const BLOCKS: &[&str] = &["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

// ── Main draw ──

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    // Title bar di atas
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Length(3), // search bar
            Constraint::Min(5),    // body
            Constraint::Length(3), // status bar
        ])
        .split(area);

    draw_titlebar(f, root[0], app);
    draw_searchbar(f, root[1], app);
    draw_body(f, root[2], app);
    draw_statusbar(f, root[3], app);
}

// ── Title bar ──

fn draw_titlebar(f: &mut Frame, area: Rect, _app: &App) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "─── YouTube CLI Player v0.1.0 ───",
            Style::default().fg(theme::OVERLAY1),
        ),
    ]))
    .alignment(Alignment::Center)
    .style(Style::default().bg(theme::CRUST));

    f.render_widget(title, area);
}

// ── Search bar ──

fn draw_searchbar(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::SURFACE2))
        .style(Style::default().bg(theme::MANTLE));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mode_tag = match app.input_mode {
        InputMode::Search => Span::styled(" SEARCH ", Style::default().fg(theme::CRUST).bg(theme::TEAL).bold()),
        InputMode::Normal => Span::styled(" NORMAL ", Style::default().fg(theme::CRUST).bg(theme::SAPPHIRE).bold()),
    };

    let cursor = if app.input_mode == InputMode::Search { "█" } else { "" };

    let label = if app.results.is_empty() {
        Span::styled("  Search: ", Style::default().fg(theme::OVERLAY1))
    } else {
        Span::styled(
            format!("  Search Results '{}' ", app.query),
            Style::default().fg(theme::TEXT).bold(),
        )
    };

    let line = Line::from(vec![
        mode_tag,
        label,
        Span::styled(&app.query, Style::default().fg(theme::TEXT)),
        Span::styled(cursor, Style::default().fg(theme::TEAL)),
    ]);

    f.render_widget(Paragraph::new(line), inner);
}

// ── Body: sidebar + content ──

fn draw_body(f: &mut Frame, area: Rect, app: &App) {
    let parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(10)])
        .split(area);

    draw_sidebar(f, parts[0], app);
    draw_content(f, parts[1], app);
}

// ── Sidebar ──

fn draw_sidebar(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::SURFACE2))
        .style(Style::default().bg(theme::BASE));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Layout dinamis: logo, [now-playing], keybinds, volume
    let has_track = app.track.is_some();
    let constraints: Vec<Constraint> = if has_track {
        vec![
            Constraint::Length(5), // logo
            Constraint::Length(5), // now playing
            Constraint::Min(3),    // keybinds (lebih pendek)
            Constraint::Length(3), // volume bar
        ]
    } else {
        vec![
            Constraint::Length(5), // logo
            Constraint::Min(8),    // keybinds
            Constraint::Length(3), // volume bar
        ]
    };

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // ── Logo ──
    let logo_lines = vec![
        Line::from(Span::styled(" ╭──────────────╮", Style::default().fg(theme::SURFACE2))),
        Line::from(Span::styled(" │  ( ▶ ) YT   │", Style::default().fg(theme::TEAL).bold())),
        Line::from(Span::styled(" ╰──────────────╯", Style::default().fg(theme::SURFACE2))),
        Line::from(Span::styled("    YT-CLI", Style::default().fg(theme::OVERLAY1))),
    ];
    f.render_widget(Paragraph::new(logo_lines), parts[0]);

    let next_part_idx = if has_track { 2 } else { 1 };
    let vol_part_idx = if has_track { 3 } else { 2 };

    // ── Now Playing (sidebar, di atas spectrum) ──
    if has_track {
        let track = app.track.as_ref().unwrap();
        let max_w = inner.width.saturating_sub(2) as usize;

        let status_icon = if track.paused {
            " ⏸"
        } else {
            " ▶"
        };

        let progress = if track.duration > 0.0 {
            let pct = (track.position / track.duration).clamp(0.0, 1.0);
            let bar_w = max_w.saturating_sub(16).max(4);
            let filled = (pct * bar_w as f64) as usize;
            let empty = bar_w.saturating_sub(filled);
            format!(
                " {} [{}{}] {:02}:{:02}",
                status_icon,
                "█".repeat(filled),
                "░".repeat(empty),
                (track.position as u64) / 60,
                (track.position as u64) % 60,
            )
        } else {
            format!(" {} Playing...", status_icon)
        };

        let np_lines = vec![
            Line::from(Span::styled(" Now Playing", Style::default().fg(theme::TEAL).bold())),
            Line::from(Span::styled(
                truncate(&track.video.title, max_w),
                Style::default().fg(theme::TEXT),
            )),
            Line::from(Span::styled(
                truncate(&track.video.channel, max_w),
                Style::default().fg(theme::SUBTEXT1),
            )),
            Line::from(Span::styled(progress, Style::default().fg(theme::SAPPHIRE))),
        ];

        f.render_widget(Paragraph::new(np_lines), parts[1]);
    }

    // ── Keybinds ──
    let has_next = app.playlist.current + 1 < app.playlist.len();
    let has_prev = app.playlist.current > 0;

    let next_style = if has_next { theme::key_style() } else { theme::dim_style() };
    let prev_style = if has_prev { theme::key_style() } else { theme::dim_style() };

    // Repeat mode label
    let repeat_label = app.repeat.label();
    let shuffle_label = if app.playlist.shuffled { "On" } else { "Off" };

    let keybind_lines = vec![
        Line::from(Span::styled(" Controls", Style::default().fg(theme::OVERLAY1))),
        Line::from(Span::styled(" [Space] Play/Pause", theme::key_style())),
        Line::from(Span::styled(" [S]  Stop", theme::key_style())),
        Line::from(vec![
            Span::styled(" [N]  Next", next_style),
        ]),
        Line::from(vec![
            Span::styled(" [P]  Prev", prev_style),
        ]),
        Line::from(Span::styled(" [V]  Vol +/-", theme::key_style())),
        Line::from(Span::styled(" [/]  Search", theme::key_style())),
        Line::from(Span::styled(" [L]  Load More", theme::key_style())),
        Line::from(Span::styled(" [←→] Seek 5s", theme::key_style())),
        Line::from(Span::styled(" [↑↓] Nav", theme::key_style())),
        Line::from(vec![
            Span::styled(" [R] Repeat: ", theme::key_style()),
            Span::styled(repeat_label, Style::default().fg(theme::TEAL).bold()),
        ]),
        Line::from(vec![
            Span::styled(" [X] Shuffle: ", theme::key_style()),
            Span::styled(shuffle_label, Style::default().fg(theme::TEAL).bold()),
        ]),
        Line::from(Span::styled(" [^C] Quit", theme::key_style())),
    ];
    f.render_widget(Paragraph::new(keybind_lines), parts[next_part_idx]);

    // ── Spectrum / Volume ──
    let vol_lines = if !app.eq_bars.is_empty() {
        let spectrum: String = app.eq_bars.iter()
            .map(|&b| BLOCKS[b as usize])
            .collect();
        vec![
            Line::from(Span::styled(" Spectrum", Style::default().fg(theme::OVERLAY1))),
            Line::from(Span::styled(spectrum, Style::default().fg(theme::TEAL))),
        ]
    } else {
        let filled = (app.volume as usize * 14) / 100;
        let empty = 14usize.saturating_sub(filled);
        let bar = format!(" [{}{}]", "#".repeat(filled), "-".repeat(empty));
        vec![
            Line::from(Span::styled(" Volume", Style::default().fg(theme::OVERLAY1))),
            Line::from(Span::styled(bar, Style::default().fg(theme::TEAL))),
        ]
    };
    f.render_widget(Paragraph::new(vol_lines), parts[vol_part_idx]);
}

// ── Content area ──

fn draw_content(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::SURFACE2))
        .style(Style::default().bg(theme::BASE));

    let inner = block.inner(area);
    f.render_widget(block, area);

    draw_results(f, inner, app);
}

fn draw_results(f: &mut Frame, area: Rect, app: &App) {
    if app.searching {
        f.render_widget(
            Paragraph::new("Searching...")
                .style(Style::default().fg(theme::SUBTEXT0))
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    if app.results.is_empty() {
        f.render_widget(
            Paragraph::new("Type a query and press Enter")
                .style(Style::default().fg(theme::SUBTEXT0))
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    if let Some(ref err) = app.error {
        f.render_widget(
            Paragraph::new(err.as_str())
                .style(Style::default().fg(theme::RED).bold()),
            area,
        );
        return;
    }

    let total = app.results.len();

    // ── Layout: list area + scrollbar ──
    let parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(1)])
        .split(area);

    let list_area = parts[0];
    let scroll_area = parts[1];

    let avail_height = list_area.height.saturating_sub(2) as usize; // 1 for header, 1 for footer

    // ── Scrollbar ──
    if total > avail_height {
        draw_scrollbar(f, scroll_area, app);
    }

    // ── Header row ──
    let avail_w = list_area.width.saturating_sub(4);
    let title_w = (avail_w as f64 * 0.45) as usize;
    let chan_w  = (avail_w as f64 * 0.25) as usize;
    let dur_w   = 9usize;
    let views_w = 10usize;
    let idx_w   = 7usize;

    // ── Visible slice (clamped biar gak panic) ──
    let scroll = app.scroll_offset.min(total.saturating_sub(1));
    let visible_count = avail_height.min(total.saturating_sub(scroll));
    let end = (scroll + visible_count).min(total);
    // Render hasil (mungkin kosong kalo terminal terlalu kecil)
    let display_rows = if scroll < total && end > scroll {
        &app.results[scroll..end]
    } else {
        &[]
    };

    // ── Render each visible row as a Paragraph line ──
    // We use a single Paragraph with all lines for simplicity/performance.
    let mut lines: Vec<Line> = Vec::with_capacity(visible_count + 2);

    // Header line
    let header_fmt = format!(
        "{:^idx$} {:^ti$} {:^ch$} {:^du$} {:^vi$}",
        "No", "Title", "Channel", "Duration", "Views",
        idx = idx_w, ti = title_w, ch = chan_w, du = dur_w, vi = views_w,
    );
    lines.push(Line::from(Span::styled(header_fmt, Style::default().fg(theme::OVERLAY1).bg(theme::SURFACE0))));

    // Result lines
    for (rel_idx, v) in display_rows.iter().enumerate() {
        let i = scroll + rel_idx;
        let selected = i == app.cursor;
        let is_current = i == app.playlist.current && app.track.is_some();

        let base = if selected {
            Style::default().bg(theme::SURFACE1).fg(theme::TEXT).bold()
        } else {
            Style::default().fg(theme::TEXT)
        };

        let idx_str = if is_current {
            format!(" ▶{:>3} ", i + 1)
        } else {
            format!(" [{:>3}]", i + 1)
        };

        let dur = if v.is_live {
            "LIVE    ".to_string()
        } else {
            format!("[{:>7}]", format_duration(v.duration))
        };

        let views_str = format!("[{:>8}]", format_views(v.views));

        let row_fmt = format!(
            "{:^idx$} {:<ti$} {:<ch$} {:>du$} {:>vi$}",
            idx_str,
            truncate(&v.title, title_w),
            truncate(&v.channel, chan_w),
            dur,
            views_str,
            idx = idx_w, ti = title_w, ch = chan_w, du = dur_w + 2, vi = views_w + 2,
        );

        // Apply styles per segment
        let span = Span::styled(row_fmt, base);
        lines.push(Line::from(span));
    }

    // ── Footer: position indicator ──
    let shown = display_rows.len();
    let end_idx = scroll + shown;
    let pos_str = if total > 0 {
        format!(
            " Results {}-{} of {} ",
            scroll + 1,
            end_idx,
            total,
        )
    } else {
        " No results ".to_string()
    };
    let extra = if total > end_idx {
        format!(" (+{} more below) ", total - end_idx)
    } else {
        String::new()
    };
    let footer = format!("{}{}", pos_str, extra);

    // Pad remaining lines so the footer sticks at the bottom
    let remaining = avail_height.saturating_sub(shown);
    for _ in 0..remaining {
        lines.push(Line::from(Span::raw("")));
    }
    lines.push(Line::from(Span::styled(
        footer,
        Style::default().fg(theme::OVERLAY1).bg(theme::SURFACE0),
    )));

    f.render_widget(Paragraph::new(lines).style(Style::default().bg(theme::BASE)), list_area);
}

/// Draw a vertical scrollbar on the right side of the results list.
fn draw_scrollbar(f: &mut Frame, area: Rect, app: &App) {
    let total = app.results.len();
    let vis_count = area.height.saturating_sub(2) as usize;
    if total == 0 || vis_count == 0 || area.height < 3 {
        return;
    }

    let scroll = app.scroll_offset.min(total.saturating_sub(1));
    let thumb_pos = if total <= vis_count {
        0
    } else {
        let max_scroll = total - vis_count;
        let ratio = if max_scroll > 0 {
            scroll as f64 / max_scroll as f64
        } else {
            0.0
        };
        let track = (area.height.saturating_sub(3)) as f64;
        (ratio * track) as u16
    };

    // Build scrollbar lines
    let mut sb_lines: Vec<Line> = Vec::with_capacity(area.height as usize);
    sb_lines.push(Line::from(Span::styled("▲", Style::default().fg(theme::OVERLAY2))));
    let track_size = area.height.saturating_sub(3) as usize;
    for i in 0..track_size {
        let bar = if i == thumb_pos as usize {
            Span::styled("█", Style::default().fg(theme::TEAL))
        } else {
            Span::styled("│", Style::default().fg(theme::SURFACE2))
        };
        sb_lines.push(Line::from(bar));
    }
    sb_lines.push(Line::from(Span::styled("▼", Style::default().fg(theme::OVERLAY2))));

    f.render_widget(Paragraph::new(sb_lines).style(Style::default().bg(theme::BASE)), area);
}

// ── Status bar ──

fn draw_statusbar(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::SURFACE2))
        .style(Style::default().bg(theme::MANTLE));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(ref track) = app.track {
        let pause_tag = if track.paused { " [PAUSED]" } else { "" };
        let line = Line::from(vec![
            Span::styled(" Playing: ", Style::default().fg(theme::OVERLAY1)),
            Span::styled(&track.video.title, Style::default().fg(theme::TEXT).bold()),
            Span::styled(" | ", Style::default().fg(theme::SURFACE2)),
            Span::styled(
                format!(
                    "{} / {}{}",
                    format_duration(track.position),
                    format_duration(track.duration),
                    pause_tag,
                ),
                Style::default().fg(theme::SAPPHIRE),
            ),
            Span::styled(" | ", Style::default().fg(theme::SURFACE2)),
            Span::styled(
                format!("Vol: {}%", app.volume),
                Style::default().fg(theme::TEAL).bold(),
            ),
        ]);
        f.render_widget(Paragraph::new(line).style(Style::default().bg(theme::MANTLE)), inner);
    } else {
        let help = match app.input_mode {
            InputMode::Search => " [Enter] Search  [↑↓] Nav  [F1] Mute  [F2/F3] Vol  [F4] Stop  [^C] Quit",
            InputMode::Normal => " [Enter/Space] Play  [N] Next  [P] Prev  [S] Stop  [R] Repeat  [X] Shuffle  [L] Load More  [/] Search  [^C] Quit",
        };
        f.render_widget(
            Paragraph::new(Span::styled(help, Style::default().fg(theme::OVERLAY1)))
                .style(Style::default().bg(theme::MANTLE)),
            inner,
        );
    }
}

// ── Helpers ──

/// Truncate string by **display width**, aman buat UTF-8 multi-byte (Korean, Jepang, emoji, dll).
fn truncate(s: &str, max: usize) -> String {
    if max < 2 {
        return s.to_string();
    }
    // Display width kalo sudah <= max, gak perlu truncate
    if s.width() <= max {
        return s.to_string();
    }

    // Potong per karakter sampe display width ≤ max-1 (buat elipsis)
    let mut out = String::with_capacity(s.len());
    let mut w = 0usize;
    let limit = max.saturating_sub(1);
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw > limit {
            break;
        }
        out.push(c);
        w += cw;
    }
    format!("{}…", out)
}

// ── Theme ──

mod theme {
    use ratatui::style::{Color, Style};

    pub const CRUST: Color    = Color::Rgb(0x11, 0x11, 0x1b);
    pub const BASE: Color     = Color::Rgb(0x1e, 0x1e, 0x2e);
    pub const MANTLE: Color   = Color::Rgb(0x18, 0x18, 0x25);
    pub const SURFACE0: Color = Color::Rgb(0x31, 0x32, 0x44);
    pub const SURFACE1: Color = Color::Rgb(0x45, 0x47, 0x5a);
    pub const SURFACE2: Color = Color::Rgb(0x58, 0x5b, 0x70);
    pub const TEXT: Color     = Color::Rgb(0xcd, 0xd6, 0xf4);
    pub const SUBTEXT0: Color = Color::Rgb(0xa6, 0xad, 0xc8);
    pub const SUBTEXT1: Color = Color::Rgb(0xba, 0xc2, 0xde);
    pub const OVERLAY1: Color = Color::Rgb(0x7f, 0x84, 0x9c);
    pub const OVERLAY2: Color = Color::Rgb(0x9c, 0xa0, 0xb0);
    pub const TEAL: Color     = Color::Rgb(0x94, 0xe2, 0xd5);
    pub const SAPPHIRE: Color = Color::Rgb(0x74, 0xc7, 0xec);
    pub const RED: Color      = Color::Rgb(0xf3, 0x8b, 0xa8);

    pub fn key_style() -> Style {
        Style::default().fg(TEXT)
    }

    pub fn dim_style() -> Style {
        Style::default().fg(OVERLAY1)
    }
}
