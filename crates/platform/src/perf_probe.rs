//! System resource sampler feeding the taskbar resource widget: a 1 Hz
//! background thread reading PDH counters into a shared snapshot, exactly
//! the `VisualizerEngine` shape — `None` in the app while the widget is
//! disarmed, so an unarmed feature costs literally nothing.
//!
//! Counter choices (Task Manager parity):
//! - CPU: `\Processor Information(_Total)\% Processor Utility` — the same
//!   counter Windows 11's Task Manager reads (plain `% Processor Time`
//!   diverges on turbo parts). Falls back to `% Processor Time` on builds
//!   lacking the Utility counter.
//! - GPU: `\GPU Engine(*)\Utilization Percentage` as a *counter array* —
//!   per-engine instances appear/die with every app, so the array API is
//!   re-read every tick. Instances group by their `luid_…` adapter tag and
//!   `engtype_3D` suffix; the busiest adapter's engine sum wins, so an
//!   iGPU+dGPU box reports the GPU actually doing work (never their sum).
//! - Net: `\Network Interface(*)\Bytes Received|Sent/sec` arrays, summed
//!   over physical interfaces (loopback/tunnel pseudo-adapters skipped).
//! - RAM: `GlobalMemoryStatusEx` — cheaper than any PDH memory counter.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;
use windows_core::{PCWSTR, w};

windows_core::link!("pdh.dll" "system" fn PdhOpenQueryW(source: PCWSTR, user_data: usize, query: *mut isize) -> i32);
windows_core::link!("pdh.dll" "system" fn PdhAddEnglishCounterW(query: isize, path: PCWSTR, user_data: usize, counter: *mut isize) -> i32);
windows_core::link!("pdh.dll" "system" fn PdhCollectQueryData(query: isize) -> i32);
windows_core::link!("pdh.dll" "system" fn PdhGetFormattedCounterValue(counter: isize, fmt: u32, ctype: *mut u32, value: *mut PdhFmtValue) -> i32);
windows_core::link!("pdh.dll" "system" fn PdhGetFormattedCounterArrayW(counter: isize, fmt: u32, size: *mut u32, count: *mut u32, buffer: *mut PdhFmtItem) -> i32);
windows_core::link!("pdh.dll" "system" fn PdhCloseQuery(query: isize) -> i32);
windows_core::link!("kernel32.dll" "system" fn GlobalMemoryStatusEx(buf: *mut MemStatusEx) -> i32);

const PDH_FMT_DOUBLE: u32 = 0x0000_0200;
const PDH_MORE_DATA: i32 = 0x8000_07D2u32 as i32;
/// `CStatus` values meaning the counter read is usable.
const PDH_CSTATUS_VALID_DATA: u32 = 0;
const PDH_CSTATUS_NEW_DATA: u32 = 1;

/// `PDH_FMT_COUNTERVALUE` — only the `doubleValue` union arm is read.
#[repr(C)]
#[derive(Clone, Copy)]
struct PdhFmtValue {
    status: u32,
    double_value: f64,
}

/// `PDH_FMT_COUNTERVALUE_ITEM_W` — per-instance name + value pair.
#[repr(C)]
#[derive(Clone, Copy)]
struct PdhFmtItem {
    name: *mut u16,
    value: PdhFmtValue,
}

#[repr(C)]
struct MemStatusEx {
    length: u32,
    memory_load: u32,
    total_phys: u64,
    avail_phys: u64,
    total_page: u64,
    avail_page: u64,
    total_virtual: u64,
    avail_virtual: u64,
    avail_extended: u64,
}

/// One 1 Hz sample — everything the widget can draw, in plain numbers.
#[derive(Clone, Copy, Debug, Default)]
pub struct PerfSnapshot {
    /// `% Processor Utility` — can exceed 100 on turbo (Task Manager
    /// shows the same overshoot); consumers clamp for display.
    pub cpu: f32,
    /// Physical memory in use, 0–100.
    pub ram: f32,
    /// Busiest adapter's `engtype_3D` engine sum, 0–100+.
    pub gpu: f32,
    pub down_bps: f64,
    pub up_bps: f64,
}

/// The running sampler: one thread at 1 Hz, joined on drop. Clone
/// [`Self::shared`] into every consumer; the engine outlives them.
pub struct PerfProbe {
    shared: Arc<RwLock<PerfSnapshot>>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

struct Counters {
    query: isize,
    cpu: isize,
    gpu: Option<isize>,
    net_rx: Option<isize>,
    net_tx: Option<isize>,
}

impl Drop for Counters {
    fn drop(&mut self) {
        unsafe {
            let _ = PdhCloseQuery(self.query);
        }
    }
}

impl Counters {
    fn open() -> Option<Self> {
        let mut query = 0isize;
        if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != 0 {
            return None;
        }
        let add = |path: PCWSTR| -> Option<isize> {
            let mut c = 0isize;
            (unsafe { PdhAddEnglishCounterW(query, path, 0, &mut c) } == 0).then_some(c)
        };
        // `% Processor Utility` (Win11 Task Manager) first; the classic
        // `% Processor Time` stays as the fallback for builds without it.
        let cpu = add(w!("\\Processor Information(_Total)\\% Processor Utility"))
            .or_else(|| add(w!("\\Processor(_Total)\\% Processor Time")))?;
        let gpu = add(w!("\\GPU Engine(*)\\Utilization Percentage"));
        let net_rx = add(w!("\\Network Interface(*)\\Bytes Received/sec"));
        let net_tx = add(w!("\\Network Interface(*)\\Bytes Sent/sec"));
        // Baseline collect: rate counters produce garbage on the first
        // read — the snapshot only lands after the second collect.
        unsafe {
            let _ = PdhCollectQueryData(query);
        }
        Some(Self {
            query,
            cpu,
            gpu,
            net_rx,
            net_tx,
        })
    }

    fn collect(&self) -> PerfSnapshot {
        unsafe {
            let _ = PdhCollectQueryData(self.query);
        }
        let mut snap = PerfSnapshot {
            cpu: counter_value(self.cpu).unwrap_or(0.0) as f32,
            ..Default::default()
        };
        let mut mem = MemStatusEx {
            length: size_of::<MemStatusEx>() as u32,
            ..unsafe { std::mem::zeroed() }
        };
        if unsafe { GlobalMemoryStatusEx(&mut mem) } != 0 {
            snap.ram = mem.memory_load as f32;
        }
        if let Some(g) = self.gpu {
            snap.gpu = gpu_percent(g) as f32;
        }
        if let Some(rx) = self.net_rx {
            snap.down_bps = counter_array_sum(rx, skip_iface);
        }
        if let Some(tx) = self.net_tx {
            snap.up_bps = counter_array_sum(tx, skip_iface);
        }
        snap
    }
}

impl PerfProbe {
    /// Spawns the 1 Hz sampler. Counter setup failures degrade to zeroed
    /// fields — a missing GPU counter shows 0 %, never a dead widget.
    pub fn new() -> Self {
        let shared = Arc::new(RwLock::new(PerfSnapshot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let join = {
            let shared = shared.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("coredesk-perf".into())
                .spawn(move || {
                    let Some(counters) = Counters::open() else {
                        tracing::warn!("perf: PdhOpenQuery failed — widget stays at zero");
                        return;
                    };
                    while !stop.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_secs(1));
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let snap = counters.collect();
                        if let Ok(mut g) = shared.write() {
                            *g = snap;
                        }
                    }
                })
                .ok()
        };
        Self { shared, stop, join }
    }

    pub fn shared(&self) -> Arc<RwLock<PerfSnapshot>> {
        self.shared.clone()
    }
}

impl Default for PerfProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PerfProbe {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn counter_value(counter: isize) -> Option<f64> {
    let mut v = PdhFmtValue {
        status: 0,
        double_value: 0.0,
    };
    let mut ty = 0u32;
    let rc = unsafe { PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, &mut ty, &mut v) };
    (rc == 0 && matches!(v.status, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA))
        .then_some(v.double_value)
        .filter(|x| x.is_finite())
}

/// Iterates a `(*)` counter's live instances — the buffer is re-queried
/// every tick because engine/interface instances churn with every app
/// start. `skip(name)` filters pseudo-instances (loopback, tunnels).
fn counter_array_map(counter: isize, skip: impl Fn(&str) -> bool) -> Vec<(String, f64)> {
    let mut size = 0u32;
    let mut count = 0u32;
    let rc = unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut size,
            &mut count,
            std::ptr::null_mut(),
        )
    };
    if rc != PDH_MORE_DATA || size == 0 {
        return Vec::new();
    }
    let n = size as usize / size_of::<PdhFmtItem>();
    let mut buf = vec![
        PdhFmtItem {
            name: std::ptr::null_mut(),
            value: PdhFmtValue {
                status: 0,
                double_value: 0.0,
            },
        };
        n
    ];
    let rc = unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut size,
            &mut count,
            buf.as_mut_ptr(),
        )
    };
    if rc != 0 {
        return Vec::new();
    }
    buf.iter()
        .take(count as usize)
        .filter(|i| !i.name.is_null())
        .filter_map(|i| {
            let name = unsafe { widestring(i.name) };
            if skip(&name) {
                return None;
            }
            matches!(
                i.value.status,
                PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
            )
            .then_some((name, i.value.double_value))
            .filter(|(_, v)| v.is_finite())
        })
        .collect()
}

unsafe fn widestring(p: *mut u16) -> String {
    unsafe {
        let mut len = 0usize;
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }
}

/// Physical interfaces only — tunnel/loopback pseudo-adapters pollute the
/// totals (and double-count on VPNs).
fn skip_iface(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.contains("isatap") || l.contains("teredo") || l.contains("loopback") || l.is_empty()
}

fn counter_array_sum(counter: isize, skip: impl Fn(&str) -> bool) -> f64 {
    counter_array_map(counter, skip)
        .into_iter()
        .map(|(_, v)| v)
        .sum()
}

/// `% GPU` for the busiest *adapter*: `engtype_3D` engine instances grouped
/// by their `luid_…` tag (iGPU and dGPU never mix), summed per adapter,
/// and the largest adapter total reported — the card doing real work.
fn gpu_percent(counter: isize) -> f64 {
    let mut per_adapter: std::collections::BTreeMap<String, f64> = Default::default();
    for (name, v) in counter_array_map(counter, |_| false) {
        if !name.ends_with("engtype_3D") {
            continue;
        }
        let Some(start) = name.find("luid_") else {
            continue;
        };
        let end = name[start..]
            .find("_phys")
            .map(|p| start + p)
            .unwrap_or(name.len());
        *per_adapter.entry(name[start..end].to_string()).or_default() += v;
    }
    per_adapter.into_values().fold(0.0, f64::max)
}
