//! WASAPI shared-mode loopback capture of the default render endpoint.
//!
//! [`LoopbackCapture`] opens whatever device Windows currently plays through —
//! speakers, headphones, HDMI — with `AUDCLNT_STREAMFLAGS_LOOPBACK`, converts
//! every packet to mono `f32` and hands it to the caller's callback on a
//! dedicated thread. The default endpoint is re-queried about once a second so
//! a headphone/monitor-speaker switch is followed without any picker UI, and
//! `set_paused` stops the stream outright (zero audio processing) while it is
//! set — the occlusion gate's kill switch.
//!
//! No `IMMNotificationClient` callback: the flat generated bindings expose no
//! `#[implement]` support, and polling `GetDefaultAudioEndpoint`/`GetId` once a
//! second is cheaper than hand-rolling a COM vtable for the same result.

use crate::wasapi_bindings::*;
use crate::{bindings, wasapi_bindings};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows_core::GUID;

/// `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT` — bindgen emits the API-set name as a
/// marker struct, so the well-known GUID constant lives here instead.
const SUBTYPE_IEEE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);
/// `KSDATAFORMAT_SUBTYPE_PCM` — see above.
const SUBTYPE_PCM: GUID = GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71);

/// ~40 ms of endpoint buffering: enough to never underrun, small enough that a
/// resume reacts to un-pausing within a frame or two of animation.
const BUFFER_HNS: i64 = 400_000;
/// Endpoint-id re-poll cadence (headphone/monitor speaker hot-swap).
const REPOLL_MS: u128 = 1_000;
/// Capture-event wait: doubles as the stop/pause poll — never blocks longer.
const WAIT_MS: u32 = 200;

/// Observable state of the running capture (written by the capture thread,
/// read by the engine/UI thread).
#[derive(Clone, Debug, Default)]
pub struct CaptureInfo {
    /// Device friendly name, e.g. "Headphones (Realtek Audio)".
    pub device_name: String,
    /// Engine mix rate in Hz (what the callback frames are sampled at).
    pub sample_rate: u32,
    /// `true` while the audio client streams (cleared while paused).
    pub running: bool,
}

#[derive(Debug)]
enum SampleFormat {
    Float32,
    Pcm16,
    Pcm24,
    Pcm32,
    /// 24 valid bits left-justified in a 32-bit container
    /// (WAVEFORMATEXTENSIBLE `wBitsPerSample=32, wValidBitsPerSample=24`).
    Pcm24In32,
}

struct Shared {
    stop: AtomicBool,
    paused: AtomicBool,
    info: Mutex<CaptureInfo>,
    /// Last error string surfaced for diagnostics (latest wins).
    error: Mutex<Option<String>>,
    /// Total mono frames delivered — health check for tests/diagnostics.
    frames: AtomicU32,
}

/// Shared-mode WASAPI loopback capture. `start` spawns the thread; `Drop`
/// signals and joins it.
pub struct LoopbackCapture {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl LoopbackCapture {
    /// Captures the default render endpoint as mono `f32` at the mix format's
    /// sample rate, invoking `on_block` per packet (~10 ms). `on_block` runs on
    /// the capture thread — it must not block.
    /// Captures the default render endpoint as mono `f32` at the mix format's
    /// sample rate, invoking `on_block` per packet (~10 ms). `on_block` runs on
    /// the capture thread — it must not block.
    ///
    /// `start` waits out the first session open (~3 s cap): callers get a real
    /// `info()` — and a synchronous `Err` when the endpoint can't be opened —
    /// instead of racing the capture thread's first `open_session`.
    pub fn start<F>(on_block: F) -> Result<Self, String>
    where
        F: FnMut(&[f32]) + Send + 'static,
    {
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            info: Mutex::new(CaptureInfo::default()),
            error: Mutex::new(None),
            frames: AtomicU32::new(0),
        });
        let inner = shared.clone();
        let thread = std::thread::Builder::new()
            .name("coredesk-wasapi".into())
            .spawn(move || capture_thread(inner, on_block))
            .map_err(|e| format!("spawn capture thread: {e}"))?;
        let me = Self {
            shared,
            thread: Some(thread),
        };
        let deadline = Instant::now() + Duration::from_millis(3000);
        while Instant::now() < deadline {
            if me.shared.info.lock().map(|g| g.sample_rate).unwrap_or(0) != 0 {
                return Ok(me);
            }
            if let Some(e) = me.last_error() {
                return Err(e);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err("first capture session did not open in 3s".into())
    }

    /// Pause/resume the stream. Paused means `IAudioClient::Stop` — the thread
    /// parks and no audio is pulled or processed at all.
    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
    }

    /// Live capture state (endpoint name, mix rate, running flag).
    pub fn info(&self) -> CaptureInfo {
        self.shared
            .info
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// The last error the capture thread recorded, if any.
    pub fn last_error(&self) -> Option<String> {
        self.shared.error.lock().ok().and_then(|g| g.clone())
    }

    /// Total delivered mono frames — a cheap liveness counter.
    pub fn frames_captured(&self) -> u32 {
        self.shared.frames.load(Ordering::Relaxed)
    }
}

impl Drop for LoopbackCapture {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// One open session: endpoint + streaming state, torn down together.
struct Session {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    device: IMMDevice,
    device_id: String,
    format: SampleFormat,
    channels: u16,
    sample_rate: u32,
    started: bool,
}

fn set_error(shared: &Shared, msg: String) {
    tracing::warn!(error = %msg, "wasapi capture");
    if let Ok(mut g) = shared.error.lock() {
        *g = Some(msg);
    }
}

fn set_running(shared: &Shared, running: bool) {
    if let Ok(mut g) = shared.info.lock() {
        g.running = running;
    }
}

/// Reads `PKEY_Device_FriendlyName` for logging; "" on any failure.
fn device_name(device: &IMMDevice) -> String {
    unsafe {
        let Ok(store) = device.OpenPropertyStore(bindings::STGM_READ as u32) else {
            return String::new();
        };
        let mut var = store
            .GetValue(&PKEY_Device_FriendlyName)
            .unwrap_or_default();
        let name = if var.Anonymous.Anonymous.vt.0 as i32 == VT_LPWSTR {
            let p = var.Anonymous.Anonymous.Anonymous.pwszVal;
            if p.is_null() {
                String::new()
            } else {
                p.to_string().unwrap_or_default()
            }
        } else {
            String::new()
        };
        let _ = PropVariantClear(&mut var);
        name
    }
}

/// `GetId` allocates the endpoint id — copy and free it.
fn take_id(pwstr: windows_core::PWSTR) -> String {
    if pwstr.is_null() {
        return String::new();
    }
    unsafe {
        let s = pwstr.to_string().unwrap_or_default();
        bindings::CoTaskMemFree(pwstr.0.cast());
        s
    }
}

/// Decodes the mix `WAVEFORMATEX` — float32, or PCM 16/24/32 (rare on shared
/// mode but legal). `None` = a format we decline to misparse.
///
/// `ptr` must point at the original `GetMixFormat` COM allocation (not a stack
/// copy): a `WAVE_FORMAT_EXTENSIBLE` tag means the buffer is really a
/// `WAVEFORMATEXTENSIBLE` (`cbSize >= 22` proves it), and only the caller's
/// original pointer carries that backing.
fn decode_format(ptr: *const WAVEFORMATEX) -> Option<(SampleFormat, u16, u32)> {
    // SAFETY: `ptr` is the live GetMixFormat buffer; reads stay within the
    // fields cbSize guarantees.
    let fmt = unsafe { &*ptr };
    let tag = fmt.wFormatTag as i32;
    if tag == WAVE_FORMAT_IEEE_FLOAT {
        return Some((SampleFormat::Float32, fmt.nChannels, fmt.nSamplesPerSec));
    }
    if tag == WAVE_FORMAT_PCM {
        return match fmt.wBitsPerSample {
            16 => Some((SampleFormat::Pcm16, fmt.nChannels, fmt.nSamplesPerSec)),
            24 => Some((SampleFormat::Pcm24, fmt.nChannels, fmt.nSamplesPerSec)),
            32 => Some((SampleFormat::Pcm32, fmt.nChannels, fmt.nSamplesPerSec)),
            _ => None,
        };
    }
    if tag == WAVE_FORMAT_EXTENSIBLE && fmt.cbSize as usize >= 22 {
        // SAFETY: cbSize proves the backing allocation is a WAVEFORMATEXTENSIBLE.
        let ext = unsafe { &*ptr.cast::<WAVEFORMATEXTENSIBLE>() };
        // The parent is 1-byte packed: copy the GUID out before comparing —
        // a reference to the packed field would be misaligned UB.
        let sub_format = ext.SubFormat;
        if sub_format == SUBTYPE_IEEE_FLOAT {
            return Some((SampleFormat::Float32, fmt.nChannels, fmt.nSamplesPerSec));
        }
        if sub_format == SUBTYPE_PCM {
            // wValidBitsPerSample decides the real layout: a 24-bit stream
            // padded into 32-bit containers is `wBitsPerSample=32` +
            // `wValidBitsPerSample=24`.
            // SAFETY: union field read — the PCM subtype guarantees the
            // ValidBits member is the live arm.
            let bits = unsafe { ext.Samples.wValidBitsPerSample };
            let container = fmt.wBitsPerSample;
            return match (container, bits) {
                (16, 16) => Some((SampleFormat::Pcm16, fmt.nChannels, fmt.nSamplesPerSec)),
                (24, 24) => Some((SampleFormat::Pcm24, fmt.nChannels, fmt.nSamplesPerSec)),
                (32, 32) => Some((SampleFormat::Pcm32, fmt.nChannels, fmt.nSamplesPerSec)),
                (32, 24) => Some((SampleFormat::Pcm24In32, fmt.nChannels, fmt.nSamplesPerSec)),
                _ => None,
            };
        }
    }
    None
}

/// Opens `eRender/eConsole` loopback on `en`. Error string on any failure.
fn open_session(en: &IMMDeviceEnumerator) -> Result<Session, String> {
    unsafe {
        let device = en
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("GetDefaultAudioEndpoint: {e}"))?;
        let device_id = take_id(device.GetId().map_err(|e| format!("GetId: {e}"))?);
        let client: IAudioClient = device
            .Activate(bindings::CLSCTX_ALL as u32, None)
            .map_err(|e| format!("IAudioClient activate: {e}"))?;
        let fmt_ptr = client
            .GetMixFormat()
            .map_err(|e| format!("GetMixFormat: {e}"))?;
        // `decode_format` AND `Initialize` read the live COM buffer — a
        // `WAVE_FORMAT_EXTENSIBLE` tag means the real struct is the 40-byte
        // WAVEFORMATEXTENSIBLE; an 18-byte WAVEFORMATEX stack copy would hand
        // both garbage for the tail (masked subtype GUID → refused format,
        // garbage channel mask → E_INVALIDARG).
        let decoded = decode_format(fmt_ptr);
        let fmt = *fmt_ptr;
        let sub = (fmt.wFormatTag == WAVE_FORMAT_EXTENSIBLE as u16 && fmt.cbSize as usize >= 22)
            .then(|| {
                // cbSize proves the buffer is a WAVEFORMATEXTENSIBLE; copying
                // the GUID avoids refs into the 1-byte packed parent.
                let ext = &*fmt_ptr.cast::<WAVEFORMATEXTENSIBLE>();
                ext.SubFormat
            });
        let Some((format, channels, sample_rate)) = decoded else {
            bindings::CoTaskMemFree(fmt_ptr.cast());
            let (tag, bits, cb) = (fmt.wFormatTag, fmt.wBitsPerSample, fmt.cbSize);
            return Err(format!(
                "unsupported mix format tag={tag} bits={bits} cb={cb} sub={sub:?}"
            ));
        };
        let init = client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            (AUDCLNT_STREAMFLAGS_LOOPBACK
                | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
                | AUDCLNT_STREAMFLAGS_NOPERSIST) as u32,
            REFERENCE_TIME(BUFFER_HNS),
            // Event-callback mode requires periodicity == buffer duration;
            // 0 is the E_INVALIDARG this loop kept tripping.
            REFERENCE_TIME(BUFFER_HNS),
            fmt_ptr,
            None,
        );
        bindings::CoTaskMemFree(fmt_ptr.cast());
        init.ok()
            .map_err(|e| format!("IAudioClient::Initialize: {e}"))?;
        let capture: IAudioCaptureClient = client
            .GetService()
            .map_err(|e| format!("IAudioCaptureClient: {e}"))?;
        tracing::info!(id = %device_id, rate = sample_rate, channels, "wasapi: loopback opened");
        Ok(Session {
            client,
            capture,
            device,
            device_id,
            format,
            channels,
            sample_rate,
            started: false,
        })
    }
}

/// Current default endpoint id, or `None` when none is configured.
fn default_endpoint_id(en: &IMMDeviceEnumerator) -> Option<String> {
    unsafe {
        en.GetDefaultAudioEndpoint(eRender, eConsole)
            .ok()
            .and_then(|d| d.GetId().ok())
            .map(take_id)
    }
}

/// Converts one capture packet to mono `f32` appended to `out`.
fn packet_to_mono(
    format: &SampleFormat,
    channels: u16,
    data: *const u8,
    frames: u32,
    silent: bool,
    out: &mut Vec<f32>,
) {
    let channels = channels.max(1) as usize;
    out.clear();
    if silent || data.is_null() {
        out.resize(frames as usize, 0.0);
        return;
    }
    out.reserve(frames as usize);
    unsafe {
        for f in 0..frames as usize {
            let base = f * channels;
            let mut sum = 0.0f32;
            match format {
                SampleFormat::Float32 => {
                    let p = data.add(base * 4).cast::<f32>();
                    for c in 0..channels {
                        sum += *p.add(c);
                    }
                }
                SampleFormat::Pcm16 => {
                    let p = data.add(base * 2).cast::<i16>();
                    for c in 0..channels {
                        sum += *p.add(c) as f32 / 32768.0;
                    }
                }
                SampleFormat::Pcm24 => {
                    for c in 0..channels {
                        let p = data.add((base + c) * 3);
                        let v =
                            (*p as i32) | ((*p.add(1) as i32) << 8) | ((*p.add(2) as i32) << 16);
                        // sign-extend 24-bit
                        let v = (v << 8) >> 8;
                        sum += v as f32 / 8_388_608.0;
                    }
                }
                SampleFormat::Pcm32 | SampleFormat::Pcm24In32 => {
                    // Pcm24In32 is MSB-aligned 24-bit in a 32-bit container —
                    // the same i32 read normalizes it correctly.
                    let p = data.add(base * 4).cast::<i32>();
                    for c in 0..channels {
                        sum += *p.add(c) as f32 / 2_147_483_648.0;
                    }
                }
            }
            out.push(sum / channels as f32);
        }
    }
}

/// Drains every queued packet; false when a fatal client error demands a
/// session reopen (device pulled/invalidated).
fn drain_packets<F>(sess: &Session, on_block: &mut F, mono: &mut Vec<f32>, shared: &Shared) -> bool
where
    F: FnMut(&[f32]),
{
    unsafe {
        loop {
            match sess.capture.GetNextPacketSize() {
                Ok(0) => return true,
                Ok(_) => {}
                Err(e) => {
                    set_error(shared, format!("GetNextPacketSize: {e}"));
                    return false;
                }
            }
            let mut data: *mut u8 = core::ptr::null_mut();
            let mut nframes = 0u32;
            let mut flags = 0u32;
            let hr = sess
                .capture
                .GetBuffer(&mut data, &mut nframes, &mut flags, None, None);
            if hr.is_err() {
                // AUDCLNT_E_DEVICE_INVALIDATED et al. — reopen the session.
                set_error(shared, format!("GetBuffer: {hr}"));
                return false;
            }
            packet_to_mono(
                &sess.format,
                sess.channels,
                data.cast_const(),
                nframes,
                flags & AUDCLNT_BUFFERFLAGS_SILENT as u32 != 0,
                mono,
            );
            let _ = sess.capture.ReleaseBuffer(nframes);
            if !mono.is_empty() {
                shared
                    .frames
                    .fetch_add(mono.len() as u32, Ordering::Relaxed);
                on_block(mono);
            }
        }
    }
}

fn capture_thread<F>(shared: Arc<Shared>, mut on_block: F)
where
    F: FnMut(&[f32]),
{
    // SAFETY: capture runs on its own MTA thread; CoUninitialize balances at exit.
    unsafe {
        let _ = bindings::CoInitializeEx(None, bindings::COINIT_MULTITHREADED as u32);
        let en: IMMDeviceEnumerator = match bindings::CoCreateInstance(
            &MMDeviceEnumerator,
            None,
            bindings::CLSCTX_ALL as u32,
        ) {
            Ok(en) => en,
            Err(e) => {
                set_error(&shared, format!("MMDeviceEnumerator: {e}"));
                return;
            }
        };
        let event = bindings::CreateEventW(None, false, false, windows_core::PCWSTR::null());
        if event.0.is_null() {
            set_error(&shared, "CreateEventW returned null".into());
            return;
        }
        let event = wasapi_bindings::HANDLE(event.0);
        let mut mono: Vec<f32> = Vec::with_capacity(8192);
        let mut session: Option<Session> = None;
        let mut last_poll = Instant::now() - Duration::from_millis(REPOLL_MS as u64);

        while !shared.stop.load(Ordering::Relaxed) {
            // Endpoint hot-swap: cheap ~1 Hz id poll + reopen on a new default.
            if last_poll.elapsed().as_millis() >= REPOLL_MS {
                last_poll = Instant::now();
                match (session.as_ref(), default_endpoint_id(&en)) {
                    (Some(s), Some(id)) if s.device_id != id => {
                        // Endpoint ids are quasi-identifying (`{guid}`) — kept out
                        // of the shareable log on purpose (non-pinned fields).
                        tracing::info!(from = %s.device_id, to = %id, "wasapi: default endpoint changed; reopening");
                        session = None;
                    }
                    _ => {}
                }
            }
            if session.is_none() {
                match open_session(&en) {
                    Ok(s) => {
                        if let Ok(mut g) = shared.info.lock() {
                            g.device_name = device_name(&s.device);
                            g.sample_rate = s.sample_rate;
                        }
                        if s.client.SetEventHandle(event).is_err() {
                            set_error(&shared, "SetEventHandle failed".into());
                        } else {
                            session = Some(s);
                        }
                    }
                    Err(e) => {
                        set_error(&shared, e);
                        // No endpoint yet (all devices unplugged) — retry shortly.
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                }
            }
            let s = match session.as_mut() {
                Some(s) => s,
                None => continue,
            };

            if shared.paused.load(Ordering::Relaxed) {
                if s.started {
                    let _ = s.client.Stop();
                    s.started = false;
                    set_running(&shared, false);
                    tracing::debug!("wasapi: stream stopped (paused)");
                }
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            if !s.started {
                if s.client.Start().is_err() {
                    set_error(&shared, "IAudioClient::Start failed".into());
                    session = None;
                    continue;
                }
                s.started = true;
                set_running(&shared, true);
                tracing::debug!("wasapi: stream started");
            }

            // Event-driven reads; the timeout keeps stop/pause responsive.
            let _ = bindings::WaitForSingleObject(bindings::HANDLE(event.0), WAIT_MS);
            if !drain_packets(s, &mut on_block, &mut mono, &shared) {
                session = None;
            }
        }
        if let Some(s) = session {
            if s.started {
                let _ = s.client.Stop();
            }
            set_running(&shared, false);
        }
        let _ = bindings::CloseHandle(bindings::HANDLE(event.0));
        bindings::CoUninitialize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_format_recognizes_shared_mode_shapes() {
        let fmt = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_IEEE_FLOAT as u16,
            nChannels: 2,
            nSamplesPerSec: 48_000,
            nAvgBytesPerSec: 384_000,
            nBlockAlign: 8,
            wBitsPerSample: 32,
            cbSize: 0,
        };
        let (kind, ch, rate) = decode_format(&fmt).unwrap();
        assert!(matches!(kind, SampleFormat::Float32));
        assert_eq!((ch, rate), (2, 48_000));

        let pcm = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            wBitsPerSample: 16,
            ..fmt
        };
        assert!(matches!(
            decode_format(&pcm).unwrap().0,
            SampleFormat::Pcm16
        ));

        // WAVEFORMATEXTENSIBLE carrying the float subtype GUID — the pointer
        // lands at `Format` (offset 0), so the extensible tail is in-bounds.
        let mut ext = WAVEFORMATEXTENSIBLE {
            Format: fmt,
            dwChannelMask: 0,
            SubFormat: SUBTYPE_IEEE_FLOAT,
            ..Default::default()
        };
        ext.Format.wFormatTag = WAVE_FORMAT_EXTENSIBLE as u16;
        ext.Format.cbSize = 22;
        assert!(matches!(
            decode_format(&ext.Format).unwrap().0,
            SampleFormat::Float32
        ));

        // Extensible PCM, 24 valid bits in a 32-bit container — the shape
        // many Realtek endpoints report.
        let mut ext24 = WAVEFORMATEXTENSIBLE {
            Format: fmt,
            dwChannelMask: 0,
            SubFormat: SUBTYPE_PCM,
            ..Default::default()
        };
        ext24.Format.wFormatTag = WAVE_FORMAT_EXTENSIBLE as u16;
        ext24.Format.cbSize = 22;
        ext24.Format.wBitsPerSample = 32;
        ext24.Samples.wValidBitsPerSample = 24;
        assert!(matches!(
            decode_format(&ext24.Format).unwrap().0,
            SampleFormat::Pcm24In32
        ));
    }

    #[test]
    fn packet_to_mono_mixes_float_channels_and_silence() {
        let stereo: [f32; 6] = [1.0, -1.0, 0.5, 0.5, 0.25, -0.75];
        let mut out = Vec::new();
        packet_to_mono(
            &SampleFormat::Float32,
            2,
            stereo.as_ptr().cast(),
            3,
            false,
            &mut out,
        );
        assert_eq!(out, vec![0.0, 0.5, -0.25]);
        packet_to_mono(
            &SampleFormat::Float32,
            2,
            core::ptr::null(),
            4,
            true,
            &mut out,
        );
        assert_eq!(out, vec![0.0; 4]);
    }

    #[test]
    fn pcm24_sign_extension() {
        // 0xFF_FF_FF = -1 in 24-bit two's complement; 0x80_00_00 = -8388608.
        let data: [u8; 6] = [0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x80];
        let mut out = Vec::new();
        packet_to_mono(&SampleFormat::Pcm24, 1, data.as_ptr(), 2, false, &mut out);
        assert!((out[0] - (-1.0 / 8_388_608.0)).abs() < 1e-9);
        assert!((out[1] - (-1.0)).abs() < 1e-6);
    }
}
