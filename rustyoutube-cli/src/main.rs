mod app;
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Cek yt-dlp tersedia
    match ytdlp::Ytdlp::check_available() {
        Ok(ver) => eprintln!("yt-dlp {} tersedia", ver),
        Err(e) => {
            eprintln!("WARNING: {}", e);
        }
    }

    // Init services
    let ytdlp = ytdlp::Ytdlp::new();
    // ytdlp.binary = cli.ytdlp_bin; // uncomment if we add builder
    let mpv = mpv::Mpv::new(&cli.socket, &cli.mpv_bin);

    // Run app
    let mut app = app::App::new(ytdlp, mpv);
    app.run()
}
