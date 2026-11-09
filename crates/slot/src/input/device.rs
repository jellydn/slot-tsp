use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use slot_input::{Btn, InputSource, Millis, PhysicalEvent, RawEvent};

use super::evdev::{
    decode, device_name, pad_map, pick_devices, to_raw, Ev, Hat, PadMap, TspAnalog, EVENT_BYTES,
    EV_ABS, EV_KEY, EV_SYN,
};
use super::trace;

const DEV: &str = "/dev/input";
const SYS: &str = "/sys/class/input";
const PSY: &str = "/sys/class/power_supply";

const LID_POLL_MS: Millis = 150;

pub struct DeviceInput {
    pending: Arc<Mutex<Vec<RawEvent>>>,
    monitor: Arc<Mutex<Option<std::collections::VecDeque<PhysicalEvent>>>>,
    hats: Vec<Arc<Mutex<Hat>>>,
    analog: Arc<Mutex<TspAnalog>>,
    analog_available: bool,
    hall: Option<PathBuf>,
    lid_shut: Option<bool>,
    next_lid_poll: Millis,
}

impl DeviceInput {
    pub fn open(root: &Path) -> Self {
        DeviceInput::open_in(Path::new(DEV), Path::new(SYS), root, trace::enabled())
    }

    pub fn open_in(dev: &Path, sys: &Path, root: &Path, trace: bool) -> Self {
        let trace = trace.then(|| Trace::start(root, dev, sys)).flatten();
        DeviceInput::open_traced(dev, sys, trace)
    }

    fn open_traced(dev: &Path, sys: &Path, trace: Option<Arc<Trace>>) -> Self {
        let pending: Arc<Mutex<Vec<RawEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let monitor = Arc::new(Mutex::new(None));
        let mut hats = Vec::new();
        let analog = Arc::new(Mutex::new(TspAnalog::default()));
        let mut analog_available = false;
        for node in pick_devices(dev, sys) {
            eprintln!(
                "slot: input {} ({})",
                node.display(),
                device_name(sys, &node)
            );
            #[cfg(not(feature = "stock-tg5040"))]
            quicken_poll(sys, &node);
            let queue = pending.clone();
            let hat = Arc::new(Mutex::new(Hat::for_map(pad_map())));
            hats.push(hat.clone());
            let name = node.display().to_string();
            let trace = trace.clone();
            let monitor = monitor.clone();
            let analog = (pad_map() == PadMap::Tsp && device_name(sys, &node) == "TRIMUI Player1")
                .then(|| analog.clone());
            analog_available |= analog.is_some();
            let spawned = std::thread::Builder::new()
                .name(format!("slot-input-{name}"))
                .spawn(move || {
                    read_node(
                        &node,
                        &queue,
                        &hat,
                        analog.as_deref(),
                        trace.as_deref(),
                        &monitor,
                    )
                });
            if let Err(e) = spawned {
                eprintln!("slot: input {name}: {e}");
            }
        }
        DeviceInput {
            pending,
            monitor,
            hats,
            analog,
            analog_available,
            hall: find_hall(Path::new(PSY)),
            lid_shut: None,
            next_lid_poll: 0,
        }
    }
}

#[cfg(not(feature = "stock-tg5040"))]
const POLL_MS: u32 = 10;

#[cfg(not(feature = "stock-tg5040"))]
fn quicken_poll(sys: &Path, node: &Path) {
    let Some(name) = node.file_name() else {
        return;
    };
    let attr = sys.join(name).join("device/poll");
    let Some(now) = std::fs::read_to_string(&attr)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
    else {
        return;
    };
    if now > POLL_MS {
        match std::fs::write(&attr, POLL_MS.to_string()) {
            Ok(()) => eprintln!(
                "slot: input {}: poll {now} ms -> {POLL_MS} ms",
                node.display()
            ),
            Err(e) => eprintln!("slot: input {}: poll: {e}", node.display()),
        }
    }
}

fn read_node(
    node: &Path,
    queue: &Mutex<Vec<RawEvent>>,
    hat: &Mutex<Hat>,
    analog: Option<&Mutex<TspAnalog>>,
    trace: Option<&Trace>,
    monitor: &Mutex<Option<std::collections::VecDeque<PhysicalEvent>>>,
) {
    let label = node
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| node.display().to_string());
    let mut file = match File::open(node) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("slot: input {}: {e}", node.display());
            return;
        }
    };
    let mut buf = [0u8; EVENT_BYTES * 16];
    loop {
        let read = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                eprintln!("slot: input {}: {e}", node.display());
                break;
            }
        };
        // Keep state changes and their queued edges in the same order as a
        // steering-policy change, even while this reader is between packets.
        let mut hat = hat.lock().unwrap_or_else(|e| e.into_inner());
        let mut events = Vec::new();
        for ev in buf[..read].chunks_exact(EVENT_BYTES).filter_map(decode) {
            if matches!(ev.kind, EV_KEY | EV_ABS) {
                if let Some(events) = monitor.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                    if events.len() == 64 {
                        events.pop_front();
                    }
                    events.push_back(PhysicalEvent {
                        node: label.clone(),
                        kind: ev.kind,
                        code: ev.code,
                        value: ev.value,
                    });
                }
            }
            if let Some(trace) = trace {
                trace.event(&label, ev);
            }
            if (ev.kind == EV_KEY && ev.value == 1) || (ev.kind == EV_ABS && ev.value != 0) {
                crate::latency::read();
            }
            match ev.kind {
                EV_ABS => {
                    if let Some(analog) = analog {
                        events.extend(analog.lock().unwrap_or_else(|e| e.into_inner()).feed(ev));
                    }
                    events.extend(hat.feed(ev));
                }
                _ => events.extend(to_raw(ev)),
            }
        }
        if events.is_empty() {
            continue;
        }
        queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(events);
    }
    if let Some(analog) = analog {
        *analog.lock().unwrap_or_else(|e| e.into_inner()) = TspAnalog::default();
        queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend([RawEvent::Up(Btn::L2), RawEvent::Up(Btn::R2)]);
    }
}

struct Trace {
    out: Mutex<File>,
    began: Instant,
}

impl Trace {
    fn start(root: &Path, dev: &Path, sys: &Path) -> Option<Arc<Trace>> {
        let dir = crate::root::logs_dir(root);
        let _ = slot_store::fs::create_dir_all(&dir);
        let path = dir.join(trace::TRACE_FILE);
        let mut file = match slot_store::fs::create_file(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("slot: {}: {e}", path.display());
                return None;
            }
        };
        for line in trace::survey(dev, sys) {
            let _ = writeln!(file, "{line}");
        }
        let _ = writeln!(file, "--");
        let _ = file.flush();
        Some(Arc::new(Trace {
            out: Mutex::new(file),
            began: Instant::now(),
        }))
    }

    fn event(&self, node: &str, ev: Ev) {
        if ev.kind == EV_SYN {
            return;
        }
        let line = trace::event_line(node, self.began.elapsed().as_millis(), ev);
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

impl DeviceInput {
    pub fn steer_with_stick(&self, on: bool) {
        for hat in &self.hats {
            let mut hat = hat.lock().unwrap_or_else(|e| e.into_inner());
            let events = hat.steer_with_stick(on);
            if !events.is_empty() {
                self.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .extend(events);
            }
        }
    }

    fn read_lid(&self) -> Option<bool> {
        let raw = std::fs::read_to_string(self.hall.as_ref()?).ok()?;
        match raw.trim() {
            "0" => Some(true),
            "1" => Some(false),
            _ => None,
        }
    }
}

impl InputSource for DeviceInput {
    fn analog_available(&self) -> bool {
        self.analog_available
    }

    fn set_monitoring(&mut self, enabled: bool) {
        let mut monitor = self.monitor.lock().unwrap_or_else(|e| e.into_inner());
        if enabled && monitor.is_none() {
            *monitor = Some(std::collections::VecDeque::new());
        } else if !enabled {
            *monitor = None;
        }
    }

    fn physical_available(&self) -> bool {
        true
    }

    fn take_physical_events(&mut self) -> Vec<PhysicalEvent> {
        self.monitor
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
            .map(|events| events.drain(..).collect())
            .unwrap_or_default()
    }

    fn analog_sticks(&self) -> [[i16; 2]; 2] {
        self.analog
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .sticks()
    }

    fn poll(&mut self, now: Millis) -> Vec<RawEvent> {
        let mut out = std::mem::take(&mut *self.pending.lock().unwrap_or_else(|e| e.into_inner()));
        if self.hall.is_some() && now >= self.next_lid_poll {
            self.next_lid_poll = now + LID_POLL_MS;
            if let Some(shut) = self.read_lid() {
                if self.lid_shut != Some(shut) {
                    self.lid_shut = Some(shut);
                    out.push(if shut {
                        RawEvent::Down(Btn::Lid)
                    } else {
                        RawEvent::Up(Btn::Lid)
                    });
                }
            }
        }
        out
    }
}

fn find_hall(psy: &Path) -> Option<PathBuf> {
    let mut supplies: Vec<PathBuf> = std::fs::read_dir(psy)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    supplies.sort();
    supplies
        .into_iter()
        .map(|d| d.join("hallkey"))
        .find(|p| p.is_file())
}
