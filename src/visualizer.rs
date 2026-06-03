/// Audio visualizer — cpal capture + rustfft → bar heights.
/// Capture dari PulseAudio monitor sink (PipeWire compat).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::mpsc;

pub const BAR_COUNT: usize = 20;

pub struct Visualizer {
    _stream: cpal::Stream,
}

impl Visualizer {
    /// Start capture + FFT. Kirim Vec<u8> (bar heights 0-7) ke tx tiap frame.
    pub fn start(tx: mpsc::Sender<Vec<u8>>) -> anyhow::Result<Self> {
        let host = cpal::default_host();

        // Cari monitor sink — device yang namanya mengandung "monitor"
        let device = Self::find_monitor(&host)?;

        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0 as f64;

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => Self::build_stream::<f32>(&device, &config.into(), tx, sample_rate)?,
            cpal::SampleFormat::I16 => Self::build_stream::<i16>(&device, &config.into(), tx, sample_rate)?,
            cpal::SampleFormat::U16 => Self::build_stream::<u16>(&device, &config.into(), tx, sample_rate)?,
            _ => anyhow::bail!("unsupported sample format"),
        };

        stream.play()?;

        Ok(Self { _stream: stream })
    }

    fn find_monitor(host: &cpal::Host) -> anyhow::Result<cpal::Device> {
        // Coba cari device dengan "monitor" di namanya
        for device in host.input_devices()? {
            if let Ok(name) = device.name() {
                if name.to_lowercase().contains("monitor") {
                    return Ok(device);
                }
            }
        }

        // Fallback: default input device
        host.default_input_device()
            .ok_or_else(|| anyhow::anyhow!("tidak ada input device ditemukan"))
    }

    fn build_stream<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        tx: mpsc::Sender<Vec<u8>>,
        _sample_rate: f64,
    ) -> anyhow::Result<cpal::Stream>
    where
        T: cpal::Sample + cpal::SizedSample + dasp_sample::ToSample<f32>,
    {
        const FFT_SIZE: usize = 1024;
        let mut buffer: Vec<f32> = Vec::with_capacity(FFT_SIZE);
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        let stream = device.build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Convert samples ke f32
                for sample in data {
                    let s: f32 = dasp_sample::Sample::to_sample(*sample);
                    buffer.push(s);
                }

                // Proses tiap FFT_SIZE samples
                while buffer.len() >= FFT_SIZE {
                    let chunk: Vec<f32> = buffer.drain(..FFT_SIZE).collect();

                    // Hann window
                    let mut fft_input: Vec<Complex<f32>> = chunk
                        .iter()
                        .enumerate()
                        .map(|(i, &s)| {
                            let window = 0.5
                                * (1.0
                                    - (2.0 * std::f32::consts::PI * i as f32
                                        / (FFT_SIZE - 1) as f32)
                                        .cos());
                            Complex::new(s * window, 0.0)
                        })
                        .collect();

                    fft.process(&mut fft_input);

                    // Ambil magnitude (nyquist = FFT_SIZE/2)
                    let nyquist = FFT_SIZE / 2;
                    let magnitudes: Vec<f32> = fft_input[..nyquist]
                        .iter()
                        .map(|c| c.norm())
                        .collect();

                    // Map ke BAR_COUNT bars (log scale sederhana)
                    let bars: Vec<u8> = (0..BAR_COUNT)
                        .map(|i| {
                            // Log-spaced frequency bins
                            let start = ((i as f32 / BAR_COUNT as f32) * nyquist as f32) as usize;
                            let end = (((i + 1) as f32 / BAR_COUNT as f32) * nyquist as f32)
                                as usize;
                            let end = end.min(nyquist).max(start + 1);

                            let avg = magnitudes[start..end].iter().sum::<f32>()
                                / (end - start) as f32;

                            // Normalize ke 0-7 (8 block chars)
                            let scaled = (avg * 0.8).sqrt() * 12.0;
                            (scaled as u8).min(7)
                        })
                        .collect();

                    let _ = tx.send(bars);
                }
            },
            |err| eprintln!("visualizer error: {}", err),
            None,
        )?;

        Ok(stream)
    }
}
