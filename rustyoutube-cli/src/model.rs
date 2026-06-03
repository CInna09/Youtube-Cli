/// Data structures — Video, Track, Playlist.

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
}

impl Playlist {
    pub fn new() -> Self {
        Self { items: Vec::new(), current: 0 }
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
