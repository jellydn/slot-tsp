mod common;

use slot::app::App;
use slot::audio::StubSink;
#[cfg(target_os = "macos")]
use slot::input::evdev::{code_to_btn_for, Ev, Hat, PadMap, TspAnalog, EV_ABS, EV_KEY};
use slot::session::Session;
use slot_input::{Action, Btn, RawEvent, MENU_HOLD_MS};
#[cfg(target_os = "macos")]
use slot_input::{InputSource, Millis, PhysicalEvent};
use slot_retro::ButtonMask;
use slot_store::{Binding, Controls, Core, Device, GameButton, Modifier};
use slot_ui::QuickRow;

fn root(core: Core, dualshock: bool) -> tempfile::TempDir {
    let d = common::tmp_root_with_carts(&[]);
    common::clocked(d.path());
    std::fs::create_dir_all(d.path().join("Games/PS")).unwrap();
    std::fs::write(
        d.path().join("Games/PS/Pad.cue"),
        "FILE \"pad.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
    )
    .unwrap();
    std::fs::copy(
        d.path().join("Games/PS/Pad.cue"),
        d.path().join("Games/PS/Zzz.cue"),
    )
    .unwrap();
    std::fs::write(d.path().join("Games/PS/pad.bin"), [0u8; 2352]).unwrap();
    slot_store::write_selected_core(d.path(), "Pad.cue", core).unwrap();
    let mut state = slot_store::read_slot_state(d.path());
    state.ps_dualshock = dualshock;
    slot_store::write_slot_state(d.path(), &state).unwrap();
    d
}

fn open(a: &mut App) {
    let press = |a: &mut App, b| {
        a.apply(Action::GbaDown(b));
        a.apply(Action::GbaUp(b));
    };
    a.apply(Action::QuickMenu);
    for _ in 0..QuickRow::Controls.position(&a.hidden_rows()) {
        press(a, Btn::Down);
    }
    press(a, Btn::A);
    for _ in 0..QuickRow::InputTest.position(&a.hidden_rows()) {
        press(a, Btn::Down);
    }
    assert_eq!(a.quick_menu(), Some(QuickRow::InputTest));
    press(a, Btn::A);
    assert!(a.input_test().is_some());
}

#[test]
fn both_core_settings_rehearse_every_ps1_button_without_loading_a_game_or_writing_bindings() {
    for core in [Core::PcsxRearmed, Core::SwanStation] {
        for dualshock in [false, true] {
            let d = root(core, dualshock);
            slot_store::write_controls(d.path(), &Controls::default()).unwrap();
            let before = std::fs::read(d.path().join("Config/controls.txt")).unwrap();
            let mut s = Session::boot_with(d.path().into(), Box::new(StubSink::new()));
            open(s.app_mut());
            let text = s.app().input_test().unwrap().lines().join("\n");
            assert!(text.contains(core.text()), "{text}");
            assert!(text.contains(if dualshock { "DualShock" } else { "Digital" }));
            assert!(text.contains("Raw evdev unavailable"));
            // Asymmetric masks and releases expose aliasing of shoulders, faces or directions.
            for (b, bit) in [
                (Btn::Up, ButtonMask::UP),
                (Btn::Down, ButtonMask::DOWN),
                (Btn::Left, ButtonMask::LEFT),
                (Btn::Right, ButtonMask::RIGHT),
                (Btn::A, ButtonMask::A),
                (Btn::X, ButtonMask::X),
                (Btn::Y, ButtonMask::Y),
                (Btn::Start, ButtonMask::START),
                (Btn::Select, ButtonMask::SELECT),
                (Btn::L1, ButtonMask::L),
                (Btn::R1, ButtonMask::R),
                (Btn::L2, ButtonMask::L2),
                (Btn::R2, ButtonMask::R2),
                (Btn::B, ButtonMask::B),
            ] {
                s.feed([RawEvent::Down(b)], 1000);
                s.feed([], 1650); // Select is deliberately deferred by the real gesture path.
                assert_eq!(
                    s.app().input_test().unwrap().mask().0,
                    bit,
                    "{core:?} {b:?}"
                );
                assert!(!s.has_core(), "the input screen loaded a game");
                s.feed([RawEvent::Up(b)], 1700);
                s.feed([], 1800);
                assert_eq!(s.app().input_test().unwrap().mask().0, 0);
            }
            s.feed([RawEvent::Down(Btn::Menu)], 2000);
            s.feed([], 2000 + MENU_HOLD_MS);
            assert!(s.app().input_test().is_none(), "Menu hold did not return");
            assert_eq!(s.app().quick_menu(), Some(QuickRow::InputTest));
            assert_eq!(
                std::fs::read(d.path().join("Config/controls.txt")).unwrap(),
                before
            );
        }
    }
}

#[test]
fn the_screen_uses_saved_remaps_and_swallows_exit_and_held_releases() {
    let d = root(Core::SwanStation, true);
    let mut controls = Controls::default();
    controls.set_game(GameButton::L, Binding::plain(Device::R2));
    controls.set_game(GameButton::R2, None);
    slot_store::write_controls(d.path(), &controls).unwrap();
    let mut s = Session::boot_with(d.path().into(), Box::new(StubSink::new()));
    open(s.app_mut());
    s.feed([RawEvent::Down(Btn::L1)], 100);
    assert_eq!(
        s.app().input_test().unwrap().mask().0,
        0,
        "echoed the physical name, not the binding"
    );
    s.feed([RawEvent::Down(Btn::R2)], 110);
    assert_eq!(s.app().input_test().unwrap().mask().0, ButtonMask::L);
    s.feed([RawEvent::Down(Btn::Menu)], 120);
    assert!(
        s.app().input_test().is_some(),
        "Menu must still work as a modifier"
    );
    s.feed([RawEvent::Up(Btn::Menu)], 125);
    assert!(s.app().input_test().is_some(), "a Menu tap exited");
    // A new hold must not inherit time from the tap, and repeated Down must not reset it.
    s.feed([RawEvent::Down(Btn::Menu)], 500);
    s.feed([RawEvent::Down(Btn::Menu)], 500 + MENU_HOLD_MS - 1);
    assert!(
        s.app().input_test().is_some(),
        "exited before the hold boundary"
    );
    s.feed([], 500 + MENU_HOLD_MS);
    assert!(s.app().input_test().is_none());
    s.feed(
        [
            RawEvent::Up(Btn::Menu),
            RawEvent::Up(Btn::L1),
            RawEvent::Up(Btn::R2),
        ],
        500 + MENU_HOLD_MS + 1,
    );
    s.feed([], 2000);
    assert_eq!(
        s.app().quick_menu(),
        Some(QuickRow::InputTest),
        "an exit release escaped"
    );
    assert!(!s.has_core());
    s.feed([RawEvent::Down(Btn::A), RawEvent::Up(Btn::A)], 2100);
    s.feed([], 3000);
    assert!(
        s.app().input_test().is_some(),
        "a previous hold survived reopen"
    );
    assert_eq!(s.app().input_test().unwrap().mask().0, 0);
}

#[test]
fn menu_bindings_are_testable_and_shortcuts_are_reported_but_never_executed() {
    let d = root(Core::PcsxRearmed, true);
    let mut controls = Controls::default();
    controls.set_game(GameButton::R, Binding::with(Modifier::Menu, Device::L1));
    slot_store::write_controls(d.path(), &controls).unwrap();
    let mut s = Session::boot_with(d.path().into(), Box::new(StubSink::new()));
    open(s.app_mut());
    s.feed([RawEvent::Down(Btn::Menu), RawEvent::Down(Btn::L1)], 100);
    assert_eq!(s.app().input_test().unwrap().mask().0, ButtonMask::R);
    s.feed([RawEvent::Up(Btn::L1), RawEvent::Up(Btn::Menu)], 110);
    assert_eq!(s.app().input_test().unwrap().mask().0, 0);
    s.feed([RawEvent::Down(Btn::Select), RawEvent::Down(Btn::R1)], 120);
    let text = s.app().input_test().unwrap().lines().join("\n");
    assert!(text.contains("SaveState (not executed)"), "{text}");
    assert!(!s.has_core());
    s.feed([RawEvent::Up(Btn::R1), RawEvent::Up(Btn::Select)], 130);
    s.feed([RawEvent::Down(Btn::Menu)], 1000);
    s.feed([RawEvent::Up(Btn::Menu)], 1100);
    assert!(s.app().input_test().is_some());
    // Start a fresh modifier outside the production double-tap window.
    s.feed([RawEvent::Down(Btn::Menu)], 1500);
    s.feed([RawEvent::Down(Btn::L1)], 1510);
    assert_eq!(s.app().input_test().unwrap().mask().0, ButtonMask::R);
    s.feed([], 1500 + MENU_HOLD_MS - 1);
    assert!(s.app().input_test().is_some());
    s.feed([RawEvent::Up(Btn::Menu)], 1500 + MENU_HOLD_MS);
    assert!(s.app().input_test().is_none());
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct AuthoredInput {
    events: Vec<RawEvent>,
    physical: Vec<PhysicalEvent>,
    analog: TspAnalog,
    hat: Hat,
    monitoring: bool,
    available: bool,
}

#[cfg(target_os = "macos")]
impl AuthoredInput {
    fn event(&mut self, kind: u16, code: u16, value: i32) {
        let ev = Ev { kind, code, value };
        self.physical.push(PhysicalEvent {
            node: "event3".into(),
            kind,
            code,
            value,
        });
        if kind == EV_ABS {
            self.events.extend(self.analog.feed(ev));
            self.events.extend(self.hat.feed(ev));
        } else if let Some(b) = code_to_btn_for(PadMap::Tsp, code) {
            self.events.push(if value == 0 {
                RawEvent::Up(b)
            } else {
                RawEvent::Down(b)
            });
        }
    }
}

#[cfg(target_os = "macos")]
impl InputSource for AuthoredInput {
    fn poll(&mut self, _: Millis) -> Vec<RawEvent> {
        std::mem::take(&mut self.events)
    }
    fn analog_sticks(&self) -> [[i16; 2]; 2] {
        self.analog.sticks()
    }
    fn analog_available(&self) -> bool {
        self.available
    }
    fn set_monitoring(&mut self, enabled: bool) {
        self.monitoring = enabled;
    }
    fn physical_available(&self) -> bool {
        self.available
    }
    fn take_physical_events(&mut self) -> Vec<PhysicalEvent> {
        std::mem::take(&mut self.physical)
    }
}

#[cfg(target_os = "macos")]
#[test]
fn authored_evdev_events_render_separately_from_ps1_actions_and_both_normalized_sticks() {
    use slot::frontend::Frontend;
    use slot_gfx::{Compositor, HeadlessSurface};
    use slot_power::SimPlatform;
    let surface = HeadlessSurface::new().expect("headless render surface");
    let mut c = Compositor::new(&surface).unwrap();
    let d = root(Core::SwanStation, false);
    let mut f = Frontend::boot_with(
        Box::new(SimPlatform::at(d.path().into())),
        Box::new(StubSink::new()),
    );
    f.upload_faces(&mut c);
    open(f.app_mut());
    let mut input = AuthoredInput::default();
    let mut now = 0;
    let capture = |f: &mut Frontend, c: &mut Compositor, name: &str, w: u32| {
        f.render(c, (w, 480));
        let rgba = c.read_frame();
        assert_eq!(rgba.len(), (w * 480 * 4) as usize);
        // Each column, normalized-axis band and exit band must have visible text, not just a background.
        for (left, right, top, bottom) in [
            (20, w / 2, 98, 282),
            (w / 2 + 10, w - 20, 98, 304),
            (20, w - 20, 334, 397),
            (20, w - 20, 414, 465),
        ] {
            let ink = (top..bottom)
                .flat_map(|y| (left..right).map(move |x| ((y * w + x) * 4) as usize))
                .filter(|&i| rgba[i] > 180 && rgba[i + 1] > 180 && rgba[i + 2] > 180)
                .count();
            assert!(ink > 40, "{name}: missing text in band {top}..{bottom}");
        }
        if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
            let file = std::fs::File::create(format!("{dir}/input-test-{name}-{w}.png")).unwrap();
            let mut e = png::Encoder::new(file, w, 480);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&rgba).unwrap();
        }
        rgba
    };
    f.advance_at(&mut input, now, 0.0);
    let waiting = capture(&mut f, &mut c, "waiting", 720);
    assert!(f
        .app()
        .input_test()
        .unwrap()
        .lines()
        .join("\n")
        .contains("Left stick: unavailable"));
    input.available = true;
    for (kind, code, value) in [
        (EV_KEY, 0x136, 1),
        (EV_ABS, 2, 128),
        (EV_ABS, 5, 127),
        (EV_ABS, 0, -16384),
        (EV_ABS, 1, 8192),
        (EV_ABS, 3, 24575),
        (EV_ABS, 4, -32767),
    ] {
        input.event(kind, code, value);
    }
    now += 10;
    f.advance_at(&mut input, now, 0.0);
    let test = f.app().input_test().unwrap();
    assert_eq!(test.mask().0, ButtonMask::L | ButtonMask::L2);
    assert_eq!(test.sticks(), [[-16384, 8192], [24575, -32767]]);
    let text = test.lines().join("\n");
    for expected in [
        "event3 ABS 0x002 = 128",
        "event3 ABS 0x005 = 127",
        "L2: HELD   R2: --",
        "X -0.50  Y +0.25",
        "X +0.75  Y -1.00",
        "Digital mode: core ignores stick axes",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    assert_ne!(waiting, capture(&mut f, &mut c, "active", 720));
    capture(&mut f, &mut c, "active", 640);
    // An unknown KEY remains visible even though it cannot activate any PS1 binding.
    for code in [0, 1, 2, 3, 4, 5] {
        input.event(EV_ABS, code, 0);
    }
    input.event(EV_KEY, 0x136, 0);
    input.event(EV_KEY, 0x2ff, 1);
    f.advance_at(&mut input, 20, 0.0);
    let test = f.app().input_test().unwrap();
    assert_eq!(test.mask().0, 0);
    assert_eq!(test.sticks(), [[0; 2]; 2]);
    assert!(test.lines().join("\n").contains("CENTER"));
    assert!(test.lines().join("\n").contains("event3 KEY 0x2ff = 1"));
    capture(&mut f, &mut c, "released", 720);
    input.events.push(RawEvent::Down(Btn::B));
    f.advance_at(&mut input, 30, 0.0);
    assert_eq!(f.app().input_test().unwrap().mask().0, ButtonMask::B);
    input.events.push(RawEvent::Up(Btn::B));
    f.advance_at(&mut input, 40, 0.0);
    assert!(input.monitoring);
    assert_eq!(f.app().input_test().unwrap().mask().0, 0);
    assert!(f
        .app()
        .input_test()
        .unwrap()
        .lines()
        .join("\n")
        .contains("Hold Menu (1 second): Back"));
    capture(&mut f, &mut c, "b-released", 720);
    input.events.push(RawEvent::Down(Btn::Menu));
    f.advance_at(&mut input, 50, 0.0);
    f.advance_at(&mut input, 50 + MENU_HOLD_MS - 1, 0.0);
    assert!(f.app().input_test().is_some());
    capture(&mut f, &mut c, "menu-held", 720);
    f.advance_at(&mut input, 50 + MENU_HOLD_MS, 0.0);
    assert!(!input.monitoring);
    assert!(f.app().input_test().is_none());

    let d = root(Core::PcsxRearmed, true);
    let mut f = Frontend::boot_with(
        Box::new(SimPlatform::at(d.path().into())),
        Box::new(StubSink::new()),
    );
    open(f.app_mut());
    let mut input = AuthoredInput {
        available: true,
        ..Default::default()
    };
    f.advance_at(&mut input, 0, 0.0);
    capture(&mut f, &mut c, "dualshock-centered", 720);
    let text = f.app().input_test().unwrap().lines().join("\n");
    assert!(text.contains("DualShock"));
    assert!(!text.contains("core ignores stick axes"));
}
