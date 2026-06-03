/// YouTube extraction via yt-dlp subprocess.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::process::Command;

use crate::model::Video;

#[derive(Debug, Deserialize)]
struct RawVideo {
    id: String,
    title: String,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    view_count: Option<i64>,
    #[serde(default)]
    live_status: Option<String>,
}

pub struct Ytdlp {
    pub binary: String,
    pub extra_args: Vec<String>,
}

impl Ytdlp {
    pub fn new() -> Self {
        Self {
            binary: "yt-dlp".into(),
            extra_args: Vec::new(),
        }
    }

    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<Video>> {
        let search = format!("ytsearch{}:{}", limit, query);

        let mut cmd = Command::new(&self.binary);
        cmd.args([
            &search,
            "--dump-json",
            "--flat-playlist",
            "--no-download",
            "--ignore-errors",
            "--no-warnings",
        ]);

        for arg in &self.extra_args {
            cmd.arg(arg);
        }

        let output = cmd
            .output()
            .with_context(|| format!("gagal menjalankan {}", self.binary))?;

        if !output.status.success() && output.stdout.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp error: {}", stderr.trim());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut videos = Vec::new();

        for line in stdout.lines() {
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<RawVideo>(line) {
                Ok(raw) => {
                    let dur = raw.duration.unwrap_or(0.0);
                    let id = raw.id;
                    let title = raw.title;
                    let channel = raw.channel;
                    let views = raw.view_count.map(|v| v.max(0) as u64).unwrap_or(0);
                    let is_live = matches!(
                        raw.live_status.as_deref(),
                        Some("is_live" | "is_upcoming")
                    );
                    videos.push(Video {
                        id,
                        title,
                        channel,
                        duration: dur,
                        views,
                        is_live,
                    });
                }
                Err(e) => {
                    eprintln!("warn: skip line: {}", e);
                }
            }
        }

        Ok(videos)
    }

    pub fn stream_url(&self, video_id: &str) -> Result<String> {
        let url = format!("https://youtube.com/watch?v={}", video_id);

        let mut cmd = Command::new(&self.binary);
        cmd.args([
            "-f", "bestaudio",
            "-g",
            "--no-download",
            "--ignore-errors",
            "--no-warnings",
            &url,
        ]);

        for arg in &self.extra_args {
            cmd.arg(arg);
        }

        let output = cmd
            .output()
            .with_context(|| format!("gagal menjalankan {}", self.binary))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp stream error: {}", stderr.trim());
        }

        let stream_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stream_url.is_empty() {
            anyhow::bail!("yt-dlp mengembalikan URL kosong untuk video {}", video_id);
        }

        Ok(stream_url)
    }

    pub fn check_available() -> Result<String> {
        let output = Command::new("yt-dlp")
            .arg("--version")
            .output()
            .context("yt-dlp tidak ditemukan. Install: https://github.com/yt-dlp/yt-dlp")?;

        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(version)
    }
}
