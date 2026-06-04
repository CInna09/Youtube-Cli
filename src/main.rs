mod app;
mod config;
mod model;
mod mpv;
mod ui;
mod ytdlp;
mod visualizer;

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(name = "rustyoutube-cli", version, about = "YouTube TUI client")]
struct Cli {
    /// Path to mpv socket
    #[arg(long, default_value = "/tmp/rustyoutube-mpv.sock")]
    socket: String,

    /// mpv binary path
    #[arg(long, default_value = "mpv")]
    mpv_bin: String,

    /// yt-dlp binary path
    #[arg(long, default_value = "yt-dlp")]
    ytdlp_bin: String,

    /// Config file path (overrides default ~/.config/rustyoutube-cli/config.toml)
    #[arg(long)]
    config: Option<String>,
}

fn main() -> Result<()> {
    let mut cli = Cli::parse();

    // Load config (custom path or default)
    let cfg = cli.config.as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok()
            .and_then(|c| toml::from_str::<config::Config>(&c).ok()))
        .or_else(|| config::Config::load());

    // Merge config into CLI (CLI default values are overridden by config)
    if let Some(ref cfg) = cfg {
        if cli.socket == "/tmp/rustyoutube-mpv.sock" {
            if let Some(ref s) = cfg.socket { cli.socket.clone_from(s); }
        }
        if cli.mpv_bin == "mpv" {
            if let Some(ref s) = cfg.mpv_bin { cli.mpv_bin.clone_from(s); }
        }
        if cli.ytdlp_bin == "yt-dlp" {
            if let Some(ref s) = cfg.ytdlp_bin { cli.ytdlp_bin.clone_from(s); }
        }
    }

    // Cek yt-dlp tersedia
    match ytdlp::Ytdlp::check_available() {
        Ok(ver) => eprintln!("yt-dlp {} tersedia", ver),
        Err(e) => {
            eprintln!("WARNING: {}", e);
        }
    }

    // Init services
    let ytdlp = ytdlp::Ytdlp::new();
    let mpv = mpv::Mpv::new(&cli.socket, &cli.mpv_bin);

    // Default volume (config → state persist → 50)
    let default_volume = cfg.as_ref()
        .and_then(|c| c.default_volume)
        .or_else(|| config::Config::load_state().map(|s| s.volume))
        .unwrap_or(50);
    let mut app = app::App::with_config(ytdlp, mpv, default_volume);
    app.run()
}
