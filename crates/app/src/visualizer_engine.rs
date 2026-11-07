//! Reactive audio-visualizer backend: WASAPI loopback capture → FFT →
//! smoothed [`SpectrumSnapshot`], gated by the desktop occlusion engine.
//!
//! One control thread owns the [`LoopbackCapture`] life cycle: while a
//! visualizer widget is active it opens the default render endpoint, and every
//! ~500 ms it re-runs the same occlusion check the effects overlay uses
//! ([`occluded`]). Occluded desktop ⇒ the stream is stopped and no audio is
//! pulled, converted or transformed at all — visualizers literally cannot be
//! seen, so they cost exactly 0 % CPU/GPU.
//!
//! Frontends (the `v1`…`v5` widget designs) read [`Self::snapshot`] at their
//! own cadence; `snapshot_json` is the WebView post format.

use coredesk_platform::wasapi::LoopbackCapture;
use coredesk_platform::{monitors, wmf_wallpaper};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Frequency bands carried by a snapshot — the fixed contract every
/// visualizer renders against.
pub const BANDS: usize = 24;
/// Radix-2 transform length — ~21 ms of audio at 48 kHz.
const FFT_N: usize = 1024;
/// Ring capacity: newest `FFT_N` are transformed; deeper history is dropped.
const RING_CAP: usize = FFT_N * 3;
/// Snapshot production cadence.
const TICK_MS: u64 = 33;
/// Occlusion re-check cadence (same 500 ms as the effects overlay).
const OCCLUSION_EVERY: u32 = 15;
/// Band edges: 60 Hz … 16 kHz — speech/bass at the low end, air at the top.
const F_LO: f32 = 60.0;
const F_HI: f32 = 16_000.0;
/// Amplitude normalization: band magnitude is compared against this dB floor.
const DB_FLOOR: f32 = 62.0;
/// Per-tick EMA: fast attack, eased release — bars jump on transients and
/// settle smoothly instead of strobing.
const ATTACK: f32 = 0.55;
const RELEASE: f32 = 0.12;

/// One video frame's worth of analysis.
#[derive(Clone, Copy, Debug)]
pub struct SpectrumSnapshot {
    /// Log-spaced band energies, 0.0 … 1.0 (already attack/release smoothed).
    pub bands: [f32; BANDS],
    /// Overall loudness, 0.0 … 1.0 RMS.
    pub level: f32,
    /// Instantaneous peak, 0.0 … 1.0.
    pub peak: f32,
    /// `true` while occlusion has the stream stopped — a frozen frame, same
    /// contract as the paused video wallpaper.
    pub paused: bool,
}

impl Default for SpectrumSnapshot {
    fn default() -> Self {
        Self {
            bands: [0.0; BANDS],
            level: 0.0,
            peak: 0.0,
            paused: false,
        }
    }
}

impl SpectrumSnapshot {
    /// WebView post form: `{"t":"vizSpectrum","bands":[…],"level":x,"peak":y,"paused":b}`.
    /// The widget host's feed timer pushes one of these per engine tick.
    pub fn json(&self) -> String {
        let mut bands = String::with_capacity(BANDS * 5);
        bands.push('[');
        for (i, b) in self.bands.iter().enumerate() {
            if i > 0 {
                bands.push(',');
            }
            bands.push_str(&format!("{b:.3}"));
        }
        bands.push(']');
        format!(
            "{{\"t\":\"vizSpectrum\",\"bands\":{bands},\"level\":{:.3},\"peak\":{:.3},\"paused\":{}}}",
            self.level, self.peak, self.paused
        )
    }
}

/// The occlusion predicate the effects overlay also runs — a fullscreen /
/// maximized foreground window or a single-monitor D3D-exclusive app.
fn occluded() -> bool {
    wmf_wallpaper::should_pause_playback()
        || (monitors::enumerate().len() <= 1
            && matches!(
                coredesk_platform::store::user_notification_state(),
                Some(3) // QUNS_RUNNING_D3D_FULL_SCREEN
            ))
}

/// Owns capture + analysis threads. `set_active(true)` starts them;
/// `set_active(false)`/`Drop` stops and joins — nothing outlives the engine.
pub struct VisualizerEngine {
    spectrum: Arc<RwLock<SpectrumSnapshot>>,
    ring: Arc<Mutex<VecDeque<f32>>>,
    stop: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl VisualizerEngine {
    pub fn new() -> Self {
        Self {
            spectrum: Arc::new(RwLock::new(SpectrumSnapshot::default())),
            ring: Arc::new(Mutex::new(VecDeque::with_capacity(RING_CAP))),
            stop: Arc::new(AtomicBool::new(false)),
            active: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }

    /// `true` ⇒ capture is armed (occlusion still gates the stream itself).
    /// `false` ⇒ the capture thread is dropped entirely and the snapshot is
    /// zeroed — an inactive visualizer holds no audio resources at all.
    pub fn set_active(&mut self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
        if active && self.thread.is_none() {
            let (spectrum, ring, stop, active) = (
                self.spectrum.clone(),
                self.ring.clone(),
                self.stop.clone(),
                self.active.clone(),
            );
            self.thread = std::thread::Builder::new()
                .name("coredesk-visualizer".into())
                .spawn(move || control_loop(spectrum, ring, stop, active))
                .ok();
        }
    }

    /// The shared read side — the widget host clones this `Arc` into its
    /// `WM_TIMER` pump and formats frames without touching the engine.
    pub fn shared(&self) -> Arc<RwLock<SpectrumSnapshot>> {
        self.spectrum.clone()
    }
}

impl Drop for VisualizerEngine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Ok(mut g) = self.spectrum.write() {
            *g = SpectrumSnapshot::default();
        }
    }
}

/// Pushes a capture packet into the analysis ring (drop-oldest bounded).
fn feed_ring(ring: &Mutex<VecDeque<f32>>, block: &[f32]) {
    let Ok(mut q) = ring.lock() else {
        return;
    };
    q.extend(block.iter().copied());
    while q.len() > RING_CAP {
        q.pop_front();
    }
}

/// Iterative radix-2 Cooley–Tukey FFT — `n` must be a power of two. Replaces
/// `re`/`im` in place with the complex spectrum.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    // Bit-reversal permutation.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let step = -2.0 * core::f32::consts::PI / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..half {
                let ang = step * k as f32;
                let (wr, wi) = (ang.cos(), ang.sin());
                let (i0, i1) = (start + k, start + k + half);
                let tr = re[i1] * wr - im[i1] * wi;
                let ti = re[i1] * wi + im[i1] * wr;
                re[i1] = re[i0] - tr;
                im[i1] = im[i0] - ti;
                re[i0] += tr;
                im[i0] += ti;
            }
        }
        len <<= 1;
    }
}

/// Hann-window the newest `FFT_N` samples of `pcm` into `re` (zero-padded
/// when fewer are available), then transform and fold the spectrum into
/// `BANDS` log-spaced bands normalized 0…1.
fn analyze(pcm: &[f32], sample_rate: u32, bands: &mut [f32; BANDS], level_peak: &mut (f32, f32)) {
    let avail = FFT_N.min(pcm.len());
    let tail = &pcm[pcm.len() - avail..];
    let mut re = [0.0f32; FFT_N];
    let mut im = [0.0f32; FFT_N];
    let mut energy = 0.0f64;
    let mut peak = 0.0f32;
    for (i, &s) in tail.iter().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * core::f32::consts::PI * i as f32 / avail.max(2) as f32).cos();
        re[i] = s * w;
        energy += (s * s) as f64;
        peak = peak.max(s.abs());
    }
    level_peak.0 = ((energy / avail.max(1) as f64).sqrt() as f32).min(1.0);
    level_peak.1 = peak.min(1.0);
    if peak <= 1e-6 {
        for b in bands.iter_mut() {
            *b = 0.0;
        }
        return;
    }
    fft(&mut re, &mut im);
    let bins = FFT_N / 2;
    let f_hi = F_HI.min(sample_rate as f32 / 2.0 - 1.0).max(F_LO * 1.5);
    for (b, slot) in bands.iter_mut().enumerate() {
        let f0 = F_LO * (f_hi / F_LO).powf(b as f32 / BANDS as f32);
        let f1 = F_LO * (f_hi / F_LO).powf((b + 1) as f32 / BANDS as f32);
        let k0 = ((f0 * FFT_N as f32 / sample_rate as f32) as usize).clamp(1, bins - 1);
        let k1 = ((f1 * FFT_N as f32 / sample_rate as f32) as usize).clamp(k0 + 1, bins);
        let mut mag = 0.0f32;
        for k in k0..k1 {
            mag = mag.max((re[k] * re[k] + im[k] * im[k]).sqrt());
        }
        let db = 20.0 * (mag / FFT_N as f32).max(1e-7).log10();
        *slot = ((db + DB_FLOOR) / DB_FLOOR).clamp(0.0, 1.0);
    }
}

/// The engine's single loop: arms/disarms capture with `active`, gates the
/// stream on occlusion, and produces a smoothed snapshot every `TICK_MS`.
fn control_loop(
    spectrum: Arc<RwLock<SpectrumSnapshot>>,
    ring: Arc<Mutex<VecDeque<f32>>>,
    stop: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
) {
    let mut capture: Option<LoopbackCapture> = None;
    let mut smoothed = [0.0f32; BANDS];
    let mut last_occl_check = Instant::now() - Duration::from_secs(1);
    let mut is_occluded = true; // assume hidden until proven otherwise
    // Starts false so the *first* occluded check still takes the
    // freeze-frame edge — booting straight into occlusion must mark the
    // snapshot paused, not leave it looking unpaused-but-dead.
    let mut was_paused = false;
    // Silence↔audible transition diagnostics ("is the engine hearing the
    // desktop?" in the log without per-frame spam): audible = level > 5 % for
    // a beat, silent = under 1 % for ~4 s.
    let mut audible = false;
    let mut silent_since: Option<Instant> = None;

    while !stop.load(Ordering::Relaxed) {
        if !active.load(Ordering::Relaxed) {
            if capture.take().is_some() {
                tracing::debug!("visualizer: capture released (inactive)");
            }
            std::thread::sleep(Duration::from_millis(120));
            continue;
        }

        if last_occl_check.elapsed().as_millis() >= (OCCLUSION_EVERY as u128 * TICK_MS as u128) {
            last_occl_check = Instant::now();
            is_occluded = occluded();
            if let Some(c) = &capture {
                c.set_paused(is_occluded);
            }
            if is_occluded && !was_paused {
                // Freeze the frame — renderers read `paused` and stop drawing.
                if let Ok(mut g) = spectrum.write() {
                    g.paused = true;
                }
            }
            was_paused = is_occluded;
        }

        if is_occluded {
            std::thread::sleep(Duration::from_millis(OCCLUSION_EVERY as u64 * TICK_MS));
            continue;
        }

        if capture.is_none() {
            let ring2 = ring.clone();
            match LoopbackCapture::start(move |blk| feed_ring(&ring2, blk)) {
                Ok(c) => {
                    c.set_paused(false);
                    capture = Some(c);
                    let info = capture.as_ref().unwrap().info();
                    tracing::info!(endpoint = %info.device_name, rate = info.sample_rate,
                        "visualizer: loopback capture armed");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "visualizer: capture failed; retrying");
                    std::thread::sleep(Duration::from_millis(1000));
                    continue;
                }
            }
        }

        // DSP tick: newest FFT_N samples → window → transform → band fold.
        let (pcm, rate) = {
            let Ok(q) = ring.lock() else {
                std::thread::sleep(Duration::from_millis(TICK_MS));
                continue;
            };
            let rate = capture
                .as_ref()
                .map(|c| c.info().sample_rate)
                .unwrap_or(48_000);
            (q.iter().copied().collect::<Vec<f32>>(), rate)
        };
        let mut bands = [0.0f32; BANDS];
        let mut level_peak = (0.0f32, 0.0f32);
        analyze(&pcm, rate.max(8_000), &mut bands, &mut level_peak);
        if !audible && level_peak.0 > 0.05 {
            audible = true;
            silent_since = None;
            tracing::debug!(level = level_peak.0, "visualizer: audio detected");
        } else if audible && level_peak.0 < 0.01 {
            let since = silent_since.get_or_insert_with(Instant::now);
            if since.elapsed() > Duration::from_secs(4) {
                audible = false;
                silent_since = None;
                tracing::debug!("visualizer: audio silent");
            }
        } else if level_peak.0 >= 0.01 {
            silent_since = None;
        }
        for i in 0..BANDS {
            let k = if bands[i] > smoothed[i] {
                ATTACK
            } else {
                RELEASE
            };
            smoothed[i] += (bands[i] - smoothed[i]) * k;
        }
        if let Ok(mut g) = spectrum.write() {
            g.bands = smoothed;
            g.level = level_peak.0;
            g.peak = level_peak.1;
            g.paused = false;
        }
        std::thread::sleep(Duration::from_millis(TICK_MS));
    }
    drop(capture);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_of_unit_impulse_is_flat() {
        let n = 64;
        let mut re = vec![0.0f32; n];
        let mut im = vec![0.0f32; n];
        re[0] = 1.0;
        fft(&mut re, &mut im);
        for k in 0..n {
            let m = (re[k] * re[k] + im[k] * im[k]).sqrt();
            assert!((m - 1.0).abs() < 1e-5, "bin {k} = {m}");
        }
    }

    #[test]
    fn analyze_places_sine_in_its_band() {
        // A 1 kHz sine at 48 kHz must land in exactly one band neighbourhood.
        let rate = 48_000u32;
        let n = FFT_N;
        let pcm: Vec<f32> = (0..n)
            .map(|i| (2.0 * core::f32::consts::PI * 1000.0 * i as f32 / rate as f32).sin())
            .collect();
        let mut bands = [0.0f32; BANDS];
        let mut lp = (0.0, 0.0);
        analyze(&pcm, rate, &mut bands, &mut lp);
        let peak_band = bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        // The band containing 1 kHz: index = BANDS * ln(1000/60)/ln(F_HI/60).
        let expect = (BANDS as f32 * (1000.0f32 / F_LO).ln() / (F_HI / F_LO).ln()) as usize;
        assert!(
            peak_band.abs_diff(expect) <= 1,
            "1 kHz landed in band {peak_band}, expected ~{expect}"
        );
        assert!(bands[peak_band] > 0.3);
        assert!(lp.0 > 0.3 && lp.1 > 0.9);
    }

    #[test]
    fn silence_yields_zero_spectrum() {
        let mut bands = [1.0f32; BANDS];
        let mut lp = (0.0, 0.0);
        analyze(&[0.0; FFT_N], 48_000, &mut bands, &mut lp);
        assert!(bands.iter().all(|&b| b == 0.0));
        assert_eq!(lp, (0.0, 0.0));
    }

    #[test]
    fn snapshot_json_shape() {
        let e = VisualizerEngine::new();
        let j = e.shared().read().map(|g| g.json()).unwrap_or_default();
        assert!(j.starts_with("{\"t\":\"vizSpectrum\""));
        assert!(j.contains("\"paused\":false"));
        assert!(j.contains("\"bands\":["));
    }
}
