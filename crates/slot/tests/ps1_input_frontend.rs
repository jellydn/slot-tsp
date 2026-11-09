//! Authored transport contract through the device/frontend path, not real-core
//! analog protocol acceptance. No renderer, real input nodes, ROM or BIOS is used.
#![cfg(target_os = "macos")]

mod common;

use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use slot::app::Phase;
use slot::audio::StubSink;
use slot::frontend::Frontend;
use slot::input::DeviceInput;
use slot_input::{Action, Context, InputSource, MENU_DOUBLE_TAP_MS};
use slot_power::SimPlatform;
use slot_retro::ButtonMask;
use slot_store::{
    Binding, Controls, Core, Device, GameButton, Modifier, Platform, Shortcut, ShortcutKeys,
    SlotState,
};
use slot_ui::QuickRow;

const AXES: [u64; 4] = [45536, 7000, 14000, 35536];

struct Replay {
    frontend: Frontend,
    input: DeviceInput,
    writer: File,
    report: PathBuf,
    last_frame: u64,
    menu_at: Option<Instant>,
}

impl Replay {
    fn tick(&mut self) {
        self.input
            .steer_with_stick(self.frontend.app().left_stick_dpad());
        self.frontend.advance(&mut self.input);
        self.frontend
            .step_emulator(Duration::from_millis(20), Duration::from_millis(100));
    }

    fn event(&mut self, kind: u16, code: u16, value: i32) {
        let mut bytes = [0u8; 24];
        bytes[16..18].copy_from_slice(&kind.to_ne_bytes());
        bytes[18..20].copy_from_slice(&code.to_ne_bytes());
        bytes[20..24].copy_from_slice(&value.to_ne_bytes());
        self.writer.write_all(&bytes).unwrap();
    }

    fn wait(&mut self, controller: u64, buttons: u16, axes: [u64; 4]) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut last = String::new();
        loop {
            self.tick();
            if let Ok(text) = fs::read_to_string(&self.report) {
                last = text;
                let values: Vec<u64> = last
                    .split_whitespace()
                    .map(|n| n.parse().unwrap())
                    .collect();
                if values.len() == 7
                    && values[0] > self.last_frame
                    && values[1] == controller
                    && values[2] == u64::from(buttons)
                    && values[3..] == axes
                {
                    self.last_frame = values[0];
                    return;
                }
            }
            assert!(Instant::now() < deadline,
                "authored frontend transport: expected controller={controller}, buttons={buttons:#06x}, axes={axes:?}; report={last:?}, phase={:?}, context={:?}",
                self.frontend.app().phase(), self.frontend.app().input_context());
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn pulse(&mut self, kind: u16, code: u16, down: i32) {
        self.event(kind, code, down);
        // Menu navigation must cross separate polls: Session selects its context
        // once per feed, not once per event in a batch.
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

    fn buttons(&mut self, controller: u64) {
        for (kind, code, down, mask) in [
            (3, 0x11, -1, ButtonMask::UP),
            (3, 0x11, 1, ButtonMask::DOWN),
            (3, 0x10, -1, ButtonMask::LEFT),
            (3, 0x10, 1, ButtonMask::RIGHT),
            (1, 0x130, 1, ButtonMask::A),
            (1, 0x131, 1, ButtonMask::B),
            (1, 0x134, 1, ButtonMask::X),
            (1, 0x133, 1, ButtonMask::Y),
            (1, 0x136, 1, ButtonMask::L),
            (1, 0x137, 1, ButtonMask::R),
            (1, 0x13b, 1, ButtonMask::START),
            (1, 0x13a, 1, ButtonMask::SELECT),
            (1, 0x138, 1, ButtonMask::L2),
            (1, 0x139, 1, ButtonMask::R2),
            (3, 2, 255, ButtonMask::L2),
            (3, 5, 255, ButtonMask::R2),
        ] {
            self.event(kind, code, down);
            self.wait(controller, mask, AXES);
            self.event(kind, code, 0);
            self.wait(controller, 0, AXES);
        }
    }

    fn toggle_controller(&mut self) {
        self.toggle_row(QuickRow::PsController);
    }

    fn toggle_row(&mut self, selected: QuickRow) {
        if let Some(at) = self.menu_at {
            while at.elapsed().as_millis() <= u128::from(MENU_DOUBLE_TAP_MS) {
                self.tick();
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        self.pulse(1, 0x13c, 1); // Menu tap opens the in-game quick menu.
        self.menu_at = Some(Instant::now());
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.frontend.app().in_game_menu().is_none() {
            self.tick();
            assert!(
                Instant::now() < deadline,
                "FIFO Menu did not open in-game menu"
            );
        }
        let mut path = vec![selected];
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
        assert_eq!(self.frontend.app().in_game_menu(), Some(selected));
        // Use the unheld direction when the stick already holds Right. Once
        // this changes mode/flag, its release must not restore digital steering.
        let direction = if self.frontend.app().left_stick_dpad() {
            -1
        } else {
            1
        };
        self.pulse(3, 0x10, direction);
        for _ in 0..4 {
            if self.frontend.app().in_game_menu().is_none() {
                break;
            }
            self.pulse(1, 0x131, 1);
        }
        assert_eq!(self.frontend.app().input_context(), Context::Game);
    }
}

fn compile(root: &Path, core: Core) {
    let fixture = if core == Core::SwanStation {
        "tests/fixtures/swanstation.c"
    } else {
        "../slot-retro/tests/fixtures/software.c"
    };
    assert!(std::process::Command::new("cc")
        .args([
            "-dynamiclib",
            "-fPIC",
            "-DSLOT_INPUT_CONTRACT",
            "-DSLOT_INPUT_FILE_CONTRACT"
        ])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture))
        .arg("-o")
        .arg(root.join("System").join(slot::core::dylib_name(core)))
        .status()
        .unwrap()
        .success());
}

fn replay(root: &Path, core: Core, state: SlotState, custom: bool, unbound: bool) -> Replay {
    slot::root::ensure(root);
    common::clocked(root);
    compile(root, core);
    let name = if core == Core::SwanStation {
        "advertised.cue"
    } else {
        "input.cue"
    };
    fs::write(
        root.join("Games/PS").join(name),
        "FILE \"input.bin\" BINARY\n TRACK 01 MODE2/2352\n INDEX 01 00:00:00\n",
    )
    .unwrap();
    fs::write(root.join("Games/PS/input.bin"), [0u8; 2352]).unwrap();
    slot_store::write_selected_core(root, name, core).unwrap();
    slot_store::write_slot_state(
        root,
        &SlotState {
            cart: Some(name.into()),
            cart_platform: Some(Platform::Ps),
            clock_set: true,
            ..state
        },
    )
    .unwrap();
    let mut controls = Controls::default();
    if unbound {
        controls.set_game(GameButton::L2, None);
        controls.set_game(GameButton::R2, None);
    }
    slot_store::write_controls(root, &controls).unwrap();
    let mut shortcuts = ShortcutKeys::default();
    if custom {
        shortcuts.set(
            Shortcut::FastForward,
            Binding::with(Modifier::Menu, Device::R2),
        );
        shortcuts.set(Shortcut::Rewind, Binding::with(Modifier::Menu, Device::L2));
    }
    slot_store::write_shortcuts(root, &shortcuts).unwrap();
    let dev = root.join("test-dev");
    let sys = root.join("test-sys");
    fs::create_dir_all(&dev).unwrap();
    let device = sys.join("event0/device");
    fs::create_dir_all(device.join("capabilities")).unwrap();
    fs::write(device.join("name"), "TRIMUI Player1\n").unwrap();
    // BTN_SOUTH (0x130) in a Linux 64-bit capability bitmap.
    fs::write(device.join("capabilities/key"), "1000000000000 0 0 0 0\n").unwrap();
    let fifo = dev.join("event0");
    let path = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    let writer = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fifo)
        .unwrap();
    let input = DeviceInput::open_in(&dev, &sys, root, false);
    assert!(
        input.analog_available(),
        "temporary TRIMUI node was not selected"
    );
    let mut frontend = Frontend::boot_with(
        Box::new(SimPlatform::at(root.into())),
        Box::new(StubSink::draining()),
    );
    frontend.set_left_stick(true);
    frontend.drive_emulator();
    assert_eq!(frontend.app().controls(), controls);
    assert_eq!(frontend.app().shortcuts(), shortcuts);
    let report = slot::persist::core_save_dir_for(root, Platform::Ps, core, name)
        .join("slot-test-input-report.txt");
    Replay {
        frontend,
        input,
        writer,
        report,
        last_frame: 0,
        menu_at: None,
    }
}

#[test]
fn fifo_frontend_ps1_authored_transport_and_persisted_bindings() {
    let _guard = common::core_lock();
    // Both tests choose the same map before creating readers. On macOS
    // open_in's hall probe has no /sys tree.
    assert!(
        !Path::new("/sys").exists(),
        "test requires a host without real sysfs"
    );
    std::env::set_var("SLOT_BOARD", "tg5040");
    for core in [Core::PcsxRearmed, Core::SwanStation] {
        for dualshock in [false, true] {
            for custom in [false, true] {
                eprintln!("authored transport: {core:?}, cold DualShock={dualshock}, custom shortcuts={custom}");
                let root = tempfile::tempdir().unwrap();
                let mut r = replay(
                    root.path(),
                    core,
                    SlotState {
                        ps_dualshock: dualshock,
                        left_stick_dpad: false,
                        ..SlotState::default()
                    },
                    custom,
                    false,
                );
                let controller = |analog: bool| {
                    if !analog {
                        1
                    } else if core == Core::SwanStation {
                        261
                    } else {
                        517
                    }
                };
                r.wait(controller(dualshock), 0, [0; 4]);
                assert!(matches!(r.frontend.app().phase(), Phase::Playing { .. }));
                assert!(!r.frontend.app().left_stick_dpad());
                for (code, value) in [(0, -20000), (1, 7000), (3, 14000), (4, -30000)] {
                    r.event(3, code, value);
                }
                r.wait(controller(dualshock), 0, AXES);
                r.buttons(controller(dualshock));
                for analog in [!dualshock, dualshock] {
                    r.toggle_controller();
                    r.wait(controller(analog), 0, AXES);
                    r.buttons(controller(analog));
                    r.event(1, 0x131, 1);
                    r.event(3, 2, 255);
                    r.event(3, 5, 255);
                    r.wait(
                        controller(analog),
                        ButtonMask::B | ButtonMask::L2 | ButtonMask::R2,
                        AXES,
                    );
                    r.event(1, 0x131, 0);
                    r.event(3, 2, 0);
                    r.event(3, 5, 0);
                    r.wait(controller(analog), 0, AXES);
                }
                r.frontend.stop();
            }
        }
        let root = tempfile::tempdir().unwrap();
        let mut r = replay(
            root.path(),
            core,
            SlotState {
                left_stick_dpad: false,
                ..SlotState::default()
            },
            false,
            true,
        );
        r.wait(1, 0, [0; 4]);
        // Diagnostic only: unbinding the triggers currently exposes default
        // shortcuts. Print their contexts without asserting an ownership policy.
        let entries = r.frontend.app().binding_table().entries();
        for action in [Action::FfStart, Action::RewindStart] {
            let active = entries
                .iter()
                .any(|e| e.action == action && e.contexts.has(Context::Game));
            eprintln!("unbound triggers diagnostic: {core:?} {action:?} active in Game={active}");
        }
        r.event(3, 2, 255);
        r.event(3, 5, 255);
        // The changed axis is a FIFO acknowledgement: a fresh zero-button
        // report alone could arrive before the reader has consumed the triggers.
        r.event(3, 0, -20000);
        r.wait(1, 0, [45536, 0, 0, 0]);
        r.event(3, 2, 0);
        r.event(3, 5, 0);
        r.event(3, 0, 0);
        r.wait(1, 0, [0; 4]);
        r.frontend.stop();
    }
}

#[test]
fn ps1_digital_stick_steering_reconciles_held_input_without_another_axis_event() {
    let _guard = common::core_lock();
    assert!(!Path::new("/sys").exists());
    std::env::set_var("SLOT_BOARD", "tg5040");
    for core in [Core::PcsxRearmed, Core::SwanStation] {
        for dualshock in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let mut r = replay(
                root.path(),
                core,
                SlotState {
                    ps_dualshock: dualshock,
                    ..SlotState::default()
                },
                false,
                false,
            );
            let controller = |dualshock: bool| {
                if !dualshock {
                    1
                } else if core == Core::SwanStation {
                    261
                } else {
                    517
                }
            };
            r.wait(controller(dualshock), 0, [0; 4]);
            r.event(3, 0, -20000);
            r.event(3, 1, 7000);
            r.event(3, 3, 14000);
            r.event(3, 4, -30000);
            r.wait(
                controller(dualshock),
                if dualshock {
                    0
                } else {
                    ButtonMask::LEFT | ButtonMask::DOWN
                },
                AXES,
            );

            // A physical hat wins over the stick; face/shoulder buttons coexist.
            r.event(3, 0x10, 1);
            r.event(3, 0x11, -1);
            r.event(1, 0x130, 1);
            r.event(1, 0x138, 1);
            r.event(1, 0x139, 1);
            let held = ButtonMask::RIGHT
                | ButtonMask::UP
                | ButtonMask::A
                | ButtonMask::L2
                | ButtonMask::R2;
            r.wait(controller(dualshock), held, AXES);
            r.frontend.set_left_stick(false);
            r.wait(controller(dualshock), held, AXES);
            r.event(3, 0x10, 0);
            r.event(3, 0x11, 0);
            r.event(1, 0x130, 0);
            r.event(1, 0x138, 0);
            r.event(1, 0x139, 0);
            r.wait(controller(dualshock), 0, AXES);
            r.frontend.set_left_stick(true);
            r.wait(
                controller(dualshock),
                if dualshock {
                    0
                } else {
                    ButtonMask::LEFT | ButtonMask::DOWN
                },
                AXES,
            );
            r.frontend.set_left_stick(false);
            r.wait(controller(dualshock), 0, AXES);
            r.event(3, 0, 0);
            r.event(3, 1, 0);
            r.frontend.set_left_stick(true);
            r.wait(controller(dualshock), 0, [0, 0, 14000, 35536]);

            // Hold a direction across real menu-driven mode changes. No new
            // stick packet is sent until both switches and flag changes finish.
            r.event(3, 0, 20000);
            let axes = [20000, 0, 14000, 35536];
            r.wait(
                controller(dualshock),
                if dualshock { 0 } else { ButtonMask::RIGHT },
                axes,
            );
            for analog in [!dualshock, dualshock] {
                r.toggle_controller();
                // A direction used to change a setting is consumed by the
                // menu. Do not replay it as gameplay on closing the menu.
                r.wait(controller(analog), 0, axes);
            }
            if dualshock {
                r.toggle_controller();
                r.wait(1, 0, axes);
            }
            assert!(!r
                .frontend
                .app()
                .in_game_hidden()
                .contains(&QuickRow::LeftStick));
            r.toggle_row(QuickRow::LeftStick);
            r.wait(1, 0, axes);
            assert!(!slot_store::read_slot_state(root.path()).left_stick_dpad);
            r.toggle_row(QuickRow::LeftStick);
            r.wait(1, 0, axes);
            assert!(slot_store::read_slot_state(root.path()).left_stick_dpad);
            r.event(3, 0, 0);
            r.wait(1, 0, [0, 0, 14000, 35536]);
            r.event(3, 0, -20000);
            r.wait(1, ButtonMask::LEFT, [45536, 0, 14000, 35536]);
            r.event(3, 0, 0);
            r.wait(1, 0, [0, 0, 14000, 35536]);
            r.frontend.stop();
        }
    }
}
