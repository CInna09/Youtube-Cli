/// Audio visualizer — cpal capture + rustfft → bar heights.
/// Capture dari PulseAudio monitor sink (PipeWire compat).
/// Improved dengan pendekatan cava:
/// - Log-spaced frequency bins (telinga manusia logaritmik)
/// - Rise/fall smoothing (rise cepat, fall lambat / gravity)
/// - Autosens (gain otomatis)
/// - Responsif & natural

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::mpsc;

pub const BAR_COUNT: usize = 20;

// ── Konstanta FFT ──
const FFT_SIZE: usize = 1024;             // 1024-point FFT → 512 magnitude bins
const MIN_FREQ: f64 = 30.0;               // cutoff rendah (Hz)
const RISE_BLEND: f32 = 0.70;              // naik cepat: 70% new, 30% prev
const FALL_BLEND: f32 = 0.15;              // turun lambat / gravity: 15% new, 85% prev
const PEAK_DECAY: f32 = 0.995;            // autosens peak decay per frame
const SENSITIVITY: f32 = 1.5;             // gain manual multiplier

pub struct Visualizer {
    _stream: cpal::Stream,
}

impl Visualizer {
    /// Start capture + FFT. Kirim Vec<u8> (bar heights 0-7) ke tx tiap frame.
    pub fn start(tx: mpsc::Sender<Vec<u8>>) -> anyhow::Result<Self> {
        let host = cpal::default_host();
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
        for device in host.input_devices()? {
            if let Ok(name) = device.name() {
                if name.to_lowercase().contains("monitor") {
                    return Ok(device);
                }
            }
        }
        host.default_input_device()
            .ok_or_else(|| anyhow::anyhow!("tidak ada input device ditemukan"))
    }

    /// Bangun log-spaced bin boundaries.
    /// Manusia dengar logaritmik — mapping [MIN_FREQ, Nyquist] ke BAR_COUNT bins.
    fn build_log_bins(sample_rate: f64) -> Vec<(usize, usize)> {
        let nyquist = FFT_SIZE / 2;
        let max_freq = sample_rate / 2.0;
        let min_log = MIN_FREQ.log10();
        let max_log = max_freq.log10();
        let log_range = max_log - min_log;

        (0..BAR_COUNT)
            .map(|i| {
                let t_start = i as f64 / BAR_COUNT as f64;
                let t_end = (i + 1) as f64 / BAR_COUNT as f64;
                let f_start = 10.0_f64.powf(min_log + log_range * t_start);
                let f_end = 10.0_f64.powf(min_log + log_range * t_end);

                let start = ((f_start / max_freq) * nyquist as f64).round() as usize;
                let end = ((f_end / max_freq) * nyquist as f64).round() as usize;

                let start = start.min(nyquist - 1);
                let end = end.clamp(start + 1, nyquist);
                (start, end)
            })
            .collect()
    }

    fn build_stream<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        tx: mpsc::Sender<Vec<u8>>,
        sample_rate: f64,
    ) -> anyhow::Result<cpal::Stream>
    where
        T: cpal::Sample + cpal::SizedSample + dasp_sample::ToSample<f32>,
    {
        let mut buffer: Vec<f32> = Vec::with_capacity(FFT_SIZE);
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        // Pre-compute log bin boundaries (depend on sample rate)
        let log_bins = Self::build_log_bins(sample_rate);

        // State untuk smoothing & autosens
        let mut smoothed = vec![0.0_f32; BAR_COUNT];
        let mut peak: f32 = 0.001;

        let stream = device.build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                for sample in data {
                    let s: f32 = dasp_sample::Sample::to_sample(*sample);
                    buffer.push(s);
                }

                while buffer.len() >= FFT_SIZE {
                    let chunk: Vec<f32> = buffer.drain(..FFT_SIZE).collect();

                    // Hann window + FFT
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

                    // Magnitude spectrum (512 bins)
                    let nyquist = FFT_SIZE / 2;
                    let magnitudes: Vec<f32> = fft_input[..nyquist]
                        .iter()
                        .map(|c| c.norm())
                        .collect();

                    // Map ke BAR_COUNT log bins + smoothing + autosens
                    let raw_bars: Vec<f32> = log_bins
                        .iter()
                        .map(|&(start, end)| {
                            let avg = magnitudes[start..end].iter().sum::<f32>()
                                / (end - start) as f32;
                            avg
                        })
                        .collect();

                    // Autosens: track peak
                    let current_max = raw_bars.iter().cloned().fold(0.0_f32, f32::max);
                    peak = (peak * PEAK_DECAY).max(current_max);
                    let gain = if peak > 0.001 {
                        SENSITIVITY / peak
                    } else {
                        SENSITIVITY
                    };

                    // Rise/fall smoothing + gain + quantize ke 0-7
                    let bars: Vec<u8> = raw_bars
                        .iter()
                        .enumerate()
                        .map(|(i, &val)| {
                            let scaled = val * gain;

                            // Smoothing: rise cepat (70% new), fall lambat / gravity (15% new)
                            if scaled >= smoothed[i] {
                                smoothed[i] = RISE_BLEND * scaled + (1.0 - RISE_BLEND) * smoothed[i];
                            } else {
                                smoothed[i] = FALL_BLEND * scaled + (1.0 - FALL_BLEND) * smoothed[i];
                            }

                            // Map ke 0-7 dengan non-linear scaling (sqrt untuk dynamic range)
                            let normalized = (smoothed[i] * 12.0).sqrt().min(7.0);
                            normalized as u8
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
