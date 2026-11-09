//! Opt-in real-core acceptance, observed only through guest-rendered SIO replies.
//! This child module can consume Session frames without adding a production API.
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::time::{Duration, Instant};

use super::Frontend;
use crate::audio::StubSink;
use crate::input::DeviceInput;
use slot_input::{Context, InputSource, MENU_DOUBLE_TAP_MS};
use slot_power::SimPlatform;
use slot_store::{Core, Platform, SlotState};
use slot_ui::QuickRow;

struct Replay {
    frontend: Frontend,
    input: DeviceInput,
    writer: File,
    sink: StubSink,
    core: Core,
    counter: u16,
    menu_at: Option<Instant>,
    _root: tempfile::TempDir,
}

impl Replay {
    fn boot(core: Core, library: &Path, content: &Path, dualshock: bool) -> Self {
        // open_in has a fixed hall-sensor probe; never run this on a device host.
        assert!(!Path::new("/sys").exists());
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        crate::root::ensure(root);
        fs::copy(
            library,
            root.join("System").join(crate::core::dylib_name(core)),
        )
        .unwrap();
        for file in ["first.cue", "first.bin"] {
            fs::copy(content.join(file), root.join("Games/PS").join(file)).unwrap();
        }
        slot_store::write_selected_core(root, "first.cue", core).unwrap();
        slot_store::write_slot_state(
            root,
            &SlotState {
                cart: Some("first.cue".into()),
                cart_platform: Some(Platform::Ps),
                clock_set: true,
                ps_dualshock: dualshock,
                left_stick_dpad: false,
                ..SlotState::default()
            },
        )
        .unwrap();
        let dev = root.join("test-dev");
        let sys = root.join("test-sys");
        fs::create_dir_all(&dev).unwrap();
        let device = sys.join("event0/device");
        fs::create_dir_all(device.join("capabilities")).unwrap();
        fs::write(device.join("name"), "TRIMUI Player1\n").unwrap();
        fs::write(device.join("capabilities/key"), "1000000000000 0 0 0 0\n").unwrap();
        let fifo = dev.join("event0");
        let path = CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        let writer = OpenOptions::new()
            .read(true)
            .write(true)
            .open(fifo)
            .unwrap();
        let input = DeviceInput::open_in(&dev, &sys, root, false);
        assert!(input.analog_available(), "run with SLOT_BOARD=tg5040");
        let sink = StubSink::new();
        let mut frontend = Frontend::boot_with(
            Box::new(SimPlatform::at(root.into())),
            Box::new(sink.clone()),
        );
        frontend.set_left_stick(true);
        frontend.drive_emulator();
        Self {
            frontend,
            input,
            writer,
            sink,
            core,
            counter: 0,
            menu_at: None,
            _root: temp,
        }
    }

    fn tick(&mut self) {
        self.sink.device_drain();
        self.input
            .steer_with_stick(self.frontend.app().left_stick_dpad());
        self.frontend.advance(&mut self.input);
        self.frontend
            .step_emulator(Duration::from_millis(20), Duration::from_millis(100));
    }

    fn event(&mut self, kind: u16, code: u16, value: i32) {
        let mut bytes = [0; 24];
        bytes[16..18].copy_from_slice(&kind.to_ne_bytes());
        bytes[18..20].copy_from_slice(&code.to_ne_bytes());
        bytes[20..24].copy_from_slice(&value.to_ne_bytes());
        self.writer.write_all(&bytes).unwrap();
    }

    fn wait(&mut self, analog: bool, buttons: u16, axes: [u8; 4]) {
        let deadline = Instant::now() + Duration::from_secs(if self.counter == 0 { 45 } else { 8 });
        let mut last = None;
        let mut geometry = None;
        let mut fresh = 0;
        loop {
            self.tick();
            if let Some(frame) = self.frontend.session.frame() {
                if geometry != Some(frame.geometry) {
                    eprintln!("guest geometry: {:?}", frame.geometry);
                    geometry = Some(frame.geometry);
                }
                if Instant::now() >= deadline {
                    capture(&frame, &format!("{:?}-failure", self.core));
                }
                if let Some(packet) = decode(&frame, frame.geometry.width, frame.geometry.height) {
                    last = Some(packet);
                    let counter = u16::from_le_bytes([packet[12], packet[13]]);
                    if matches!(counter.wrapping_sub(self.counter), 1..=0x7fff) {
                        self.counter = counter;
                        let matches = packet[2] == 0xff
                            && packet[3] == if analog { 0x73 } else { 0x41 }
                            && packet[4] == 0x5a
                            && packet[11] == if analog { 9 } else { 5 }
                            && u16::from_le_bytes([packet[5], packet[6]]) == !buttons
                            && packet[7..11]
                                .iter()
                                .zip(axes)
                                .all(|(&a, b)| a.abs_diff(b) <= 1);
                        fresh = if matches { fresh + 1 } else { 0 };
                        if fresh == 2 {
                            eprintln!("guest SIO: analog={analog}, buttons={buttons:04x}, packet={packet:02x?}");
                            if !analog || (axes != [128; 4] && axes != [0; 4]) {
                                capture(&frame, &format!("{:?}-{:02x}", self.core, packet[3]));
                            }
                            return;
                        }
                    }
                }
            }
            assert!(Instant::now() < deadline,
                "guest SIO: expected analog={analog}, buttons={buttons:04x}, axes={axes:?}; last={last:02x?}, phase={:?}, context={:?}",
                self.frontend.app().phase(), self.frontend.app().input_context());
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn pulse(&mut self, kind: u16, code: u16, value: i32) {
        self.event(kind, code, value);
        for _ in 0..5 {
            self.tick();
            std::thread::sleep(Duration::from_millis(2));
        }
        self.event(kind, code, 0);
        for _ in 0..5 {
            self.tick();
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn switch_controller(&mut self) {
        if let Some(at) = self.menu_at {
            while at.elapsed().as_millis() <= u128::from(MENU_DOUBLE_TAP_MS) {
                self.tick();
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        self.pulse(1, 0x13c, 1);
        self.menu_at = Some(Instant::now());
        assert!(self.frontend.app().in_game_menu().is_some());
        let mut path = vec![QuickRow::PsController];
        while let Some(parent) = path.last().and_then(|row| row.parent_in_game()) {
            path.push(parent);
        }
        for (depth, row) in path.iter().rev().enumerate() {
            if depth > 0 {
                self.pulse(1, 0x130, 1);
            }
            for _ in 0..row.position(&self.frontend.app().in_game_hidden()) {
                self.pulse(3, 0x11, 1);
            }
        }
        assert_eq!(
            self.frontend.app().in_game_menu(),
            Some(QuickRow::PsController)
        );
        self.pulse(3, 0x10, 1);
        for _ in 0..4 {
            if self.frontend.app().in_game_menu().is_none() {
                break;
            }
            self.pulse(1, 0x131, 1);
        }
        assert_eq!(self.frontend.app().input_context(), Context::Game);
    }

    fn reset(&mut self) {
        let prior = self.counter;
        self.frontend
            .session
            .app_mut()
            .apply(slot_input::Action::Reset);
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            self.tick();
            if let Some(frame) = self.frontend.session.frame() {
                if let Some(packet) = decode(&frame, frame.geometry.width, frame.geometry.height) {
                    let counter = u16::from_le_bytes([packet[12], packet[13]]);
                    if counter > 0 && counter < prior {
                        self.counter = counter;
                        break;
                    }
                }
            }
            assert!(
                Instant::now() < deadline,
                "guest poll counter did not restart after reset"
            );
        }
        self.wait(true, 0, [128; 4]);
    }

    fn load_state(&mut self, analog: bool, axes: [u8; 4]) {
        let prior = self.counter;
        let supported = self.frontend.app().may_load_state();
        self.frontend
            .session
            .app_mut()
            .apply(slot_input::Action::LoadState);
        if !supported {
            // Swan deliberately refuses states until card reconciliation is safe.
            // A refused shortcut must leave the selected controller usable.
            eprintln!("guest state load refused by policy: {:?}", self.core);
            self.wait(analog, 0, axes);
            return;
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            self.tick();
            if let Some(frame) = self.frontend.session.frame() {
                if let Some(packet) = decode(&frame, frame.geometry.width, frame.geometry.height) {
                    let counter = u16::from_le_bytes([packet[12], packet[13]]);
                    if counter > 0 && counter < prior {
                        self.counter = counter;
                        break;
                    }
                }
            }
            assert!(Instant::now() < deadline, "guest state did not load");
        }
        self.wait(analog, 0, axes);
    }

    fn buttons(&mut self, analog: bool, axes: [u8; 4]) {
        // Actual active-low PS1 wire positions, not the frontend's ButtonMask.
        for (kind, code, value, bit) in [
            (3, 0x11, -1, 4),
            (3, 0x11, 1, 6),
            (3, 0x10, -1, 7),
            (3, 0x10, 1, 5),
            (1, 0x130, 1, 13),
            (1, 0x131, 1, 14),
            (1, 0x134, 1, 12),
            (1, 0x133, 1, 15),
            (1, 0x136, 1, 10),
            (1, 0x137, 1, 11),
            (1, 0x13b, 1, 3),
            (1, 0x13a, 1, 0),
            (1, 0x138, 1, 8),
            (1, 0x139, 1, 9),
            (3, 2, 255, 8),
            (3, 5, 255, 9),
        ] {
            self.event(kind, code, value);
            self.wait(analog, 1 << bit, axes);
            self.event(kind, code, 0);
            self.wait(analog, 0, axes);
        }
    }
}

fn decode(frame: &[u8], width: u32, height: u32) -> Option<[u8; 15]> {
    if width != 320 || !matches!(height, 224 | 240) || frame.len() != (width * height * 4) as usize
    {
        return None;
    }
    let mut packet = [0; 15];
    for (row, byte) in packet.iter_mut().enumerate() {
        for bit in 0..8 {
            let offset = ((row * 8 + 52) * 320 + 24 + bit * 32) * 4;
            let pixel = &frame[offset..offset + 3];
            if pixel.iter().all(|&v| v >= 192) {
                *byte |= 1 << bit;
            } else if !pixel.iter().all(|&v| v < 16) {
                return None;
            }
        }
    }
    (packet[..2] == [0xa6, 0x59] && packet.iter().fold(0, |xor, b| xor ^ b) == 0).then_some(packet)
}

fn capture(frame: &crate::frames::FrameRef, name: &str) {
    let Some(dir) = std::env::var_os("SLOT_PS1_ANALOG_CAPTURE") else {
        return;
    };
    let file = File::create(Path::new(&dir).join(format!("{name}.png"))).unwrap();
    let mut encoder = png::Encoder::new(file, frame.geometry.width, frame.geometry.height);
    encoder.set_color(png::ColorType::Rgb);
    let rgb: Vec<u8> = frame
        .chunks_exact(4)
        .flat_map(|p| [p[2], p[1], p[0]])
        .collect();
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgb)
        .unwrap();
}

fn acceptance(core: Core, key: &str) {
    let content = std::env::var_os("SLOT_PS1_ANALOG_DIR").expect("build analog-acceptance.S first");
    let library = std::env::var_os(key).expect(key);
    for cold_analog in [false, true] {
        eprintln!("real frontend acceptance: {core:?}, cold_analog={cold_analog}");
        let mut r = Replay::boot(core, Path::new(&library), Path::new(&content), cold_analog);
        r.wait(cold_analog, 0, if cold_analog { [128; 4] } else { [0; 4] });
        r.buttons(cold_analog, if cold_analog { [128; 4] } else { [0; 4] });
        if !cold_analog {
            r.frontend
                .session
                .app_mut()
                .apply(slot_input::Action::SaveState);
            r.switch_controller();
            r.wait(true, 0, [128; 4]);
        }
        for (code, index) in [(0, 2), (1, 3), (3, 0), (4, 1)] {
            for (value, expected) in [(-32767, 0), (32767, 255)] {
                let mut axes = [128; 4];
                axes[index] = expected;
                r.event(3, code, value);
                r.wait(true, 0, axes);
                r.event(3, code, 0);
                r.wait(true, 0, [128; 4]);
            }
        }
        for (code, value) in [(0, -24576), (1, 8192), (3, 16384), (4, -12288)] {
            r.event(3, code, value);
        }
        // Pinned defaults: PCSX round((axis >> 8) / 0.785) + 128;
        // Swan round((axis / 32767 * 1.33 + 1) / 2 * 255), clamped.
        let mixed = if core == Core::SwanStation {
            [212, 64, 0, 170]
        } else {
            [210, 67, 6, 169]
        };
        r.wait(true, 0, mixed);
        r.buttons(true, mixed);
        if cold_analog {
            r.frontend
                .session
                .app_mut()
                .apply(slot_input::Action::SaveState);
        } else {
            r.load_state(true, mixed);
            r.buttons(true, mixed);
        }
        r.switch_controller();
        r.wait(false, 0, [0; 4]);
        r.buttons(false, [0; 4]);
        if cold_analog {
            r.load_state(false, [0; 4]);
            r.buttons(false, [0; 4]);
        }
        r.switch_controller();
        r.wait(true, 0, mixed);
        for code in [0, 1, 3, 4] {
            r.event(3, code, 0);
        }
        r.wait(true, 0, [128; 4]);
        r.reset();
        r.buttons(true, [128; 4]);
    }
}

#[test]
#[ignore = "requires SLOT_BOARD=tg5040, SLOT_PCSX_CORE and SLOT_PS1_ANALOG_DIR; run serially"]
fn pcsx_real_core_analog_acceptance() {
    acceptance(Core::PcsxRearmed, "SLOT_PCSX_CORE");
}

#[test]
#[ignore = "requires SLOT_BOARD=tg5040, SLOT_SWAN_CORE and SLOT_PS1_ANALOG_DIR; run serially"]
fn swan_real_core_analog_acceptance() {
    acceptance(Core::SwanStation, "SLOT_SWAN_CORE");
}
