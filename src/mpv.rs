/// mpv control via Unix socket JSON-RPC.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Serialize)]
struct Request {
    command: Vec<serde_json::Value>,
    request_id: u32,
}

#[derive(Deserialize)]
struct Response {
    error: Option<String>,
    data: Option<serde_json::Value>,
}

pub struct Mpv {
    socket_path: String,
    binary: String,
    process: Option<Child>,
    conn: Mutex<Option<UnixStream>>,
}

impl Mpv {
    pub fn new(socket_path: &str, binary: &str) -> Self {
        Self {
            socket_path: socket_path.into(),
            binary: binary.into(),
            process: None,
            conn: Mutex::new(None),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        let _ = std::fs::remove_file(&self.socket_path);

        let mut cmd = Command::new(&self.binary);
        cmd.args([
            "--idle",
            "--no-terminal",
            "--no-video",
            "--no-audio-display",
            "--volume=100",
            "--volume-max=100",
            "--replaygain=no",
            "--af=scaletempo",
            &format!("--input-ipc-server={}", self.socket_path),
        ]);

        let child = cmd.spawn().context("gagal start mpv")?;
        self.process = Some(child);

        for _ in 0..50 {
            if std::path::Path::new(&self.socket_path).exists() {
                return self.connect();
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        anyhow::bail!("mpv socket tidak muncul setelah 5s di {}", self.socket_path);
    }

    fn connect(&self) -> Result<()> {
        let stream = UnixStream::connect(&self.socket_path)
            .context("gagal connect ke mpv socket")?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        *self.conn.lock().unwrap() = Some(stream);
        Ok(())
    }

    fn send(&self, args: &[serde_json::Value]) -> Result<Option<serde_json::Value>> {
        let req = Request {
            command: args.to_vec(),
            request_id: 1,
        };

        let mut json = serde_json::to_string(&req)?;
        json.push('\n');

        let mut guard = self.conn.lock().unwrap();
        let stream = guard.as_mut().context("mpv belum connect")?;

        stream.write_all(json.as_bytes())?;
        stream.flush()?;

        let mut line = String::new();
        let mut reader = BufReader::new(stream.by_ref());
        reader.read_line(&mut line)?;

        if line.is_empty() {
            anyhow::bail!("koneksi mpv terputus");
        }

        let resp: Response = serde_json::from_str(line.trim())?;

        if let Some(err) = resp.error {
            if err != "success" && !err.is_empty() {
                anyhow::bail!("mpv error: {}", err);
            }
        }

        Ok(resp.data)
    }

    // ── High-level API ──

    pub fn load_url(&self, url: &str) -> Result<()> {
        self.send(&["loadfile".into(), url.into(), "replace".into()])?;
        Ok(())
    }

    pub fn set_pause(&self, paused: bool) -> Result<()> {
        self.send(&["set_property".into(), "pause".into(), paused.into()])?;
        Ok(())
    }

    pub fn toggle_pause(&self) -> Result<bool> {
        let data = self.send(&["get_property".into(), "pause".into()])?;
        let current = data.and_then(|v| v.as_bool()).unwrap_or(false);
        self.set_pause(!current)?;
        Ok(!current)
    }

    pub fn set_volume(&self, vol: u8) -> Result<()> {
        let v: u32 = (vol as u32).min(100);
        self.send(&["set_property".into(), "volume".into(), v.into()])?;
        Ok(())
    }

    pub fn get_volume(&self) -> Result<u8> {
        let data = self.send(&["get_property".into(), "volume".into()])?;
        let vol = data
            .and_then(|v| v.as_f64())
            .map(|v| v as u8)
            .context("mpv: volume property returned null")?;
        Ok(vol)
    }

    pub fn get_position(&self) -> Result<f64> {
        let data = self.send(&["get_property".into(), "time-pos".into()])?;
        Ok(data.and_then(|v| v.as_f64()).unwrap_or(0.0))
    }

    pub fn get_duration(&self) -> Result<f64> {
        let data = self.send(&["get_property".into(), "duration".into()])?;
        Ok(data.and_then(|v| v.as_f64()).unwrap_or(0.0))
    }

    pub fn seek(&self, seconds: f64) -> Result<()> {
        self.send(&["seek".into(), seconds.into(), "relative".into()])?;
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        let _ = self.send(&["stop".into()]);
        Ok(())
    }

    pub fn get_paused(&self) -> Result<bool> {
        let data = self.send(&["get_property".into(), "pause".into()])?;
        Ok(data.and_then(|v| v.as_bool()).unwrap_or(false))
    }

    pub fn get_eof_reached(&self) -> Result<bool> {
        let data = self.send(&["get_property".into(), "eof-reached".into()])?;
        Ok(data.and_then(|v| v.as_bool()).unwrap_or(false))
    }

    pub fn quit(&mut self) {
        let _ = self.send(&["quit".into()]);
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        *self.conn.lock().unwrap() = None;
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        self.quit();
    }
}
