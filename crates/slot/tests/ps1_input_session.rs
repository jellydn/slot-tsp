mod common;

use std::time::{Duration, Instant};

use slot::app::Phase;
use slot::audio::StubSink;
use slot::emu::Speed;
use slot::session::Session;
use slot_input::{Btn, RawEvent};
use slot_retro::ButtonMask;
use slot_store::{Core, Platform, SlotState};
use slot_ui::QuickRow;

fn toggle_controller(session: &mut Session) {
    let app = session.app_mut();
    let press = |app: &mut slot::app::App, button| {
        app.apply(slot_input::Action::GbaDown(button));
        app.apply(slot_input::Action::GbaUp(button));
    };
    app.apply(slot_input::Action::QuickMenu);
    let mut path = vec![QuickRow::PsController];
    while let Some(parent) = path.last().and_then(|r| r.parent_in_game()) {
        path.push(parent);
    }
    for (depth, row) in path.iter().rev().enumerate() {
        if depth > 0 {
            press(app, Btn::A);
        }
        for _ in 0..row.position(&app.in_game_hidden()) {
            press(app, Btn::Down);
        }
    }
    assert_eq!(app.in_game_menu(), Some(QuickRow::PsController));
    press(app, Btn::Right);
    for _ in 0..4 {
        if app.in_game_menu().is_none() {
            break;
        }
        press(app, Btn::B);
    }
    assert!(app.in_game_menu().is_none());
}

fn echo(session: &mut Session, controller: u32, buttons: u16, sticks: [u16; 4]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        session.update(0.02);
        if let Some(frame) = session.frame() {
            let pixel = |n| u32::from_ne_bytes(frame[n * 4..n * 4 + 4].try_into().unwrap());
            if pixel(1) == controller
                && pixel(6) == u32::from(buttons)
                && (0..4).all(|i| pixel(i + 2) == u32::from(sticks[i]))
            {
                assert_eq!(session.observed_speed(), Some(Speed::Normal));
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "Session did not deliver controller={controller}, buttons={buttons:#06x}, sticks={sticks:?}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn both_ps1_core_routes_deliver_buttons_and_sticks_without_plain_trigger_shortcuts() {
    let _guard = common::core_lock();
    for core in [Core::PcsxRearmed, Core::SwanStation] {
        for dualshock in [false, true] {
            let d = tempfile::tempdir().unwrap();
            let root = d.path();
            slot::root::ensure(root);
            common::clocked(root);
            let fixture = if core == Core::SwanStation {
                "tests/fixtures/swanstation.c"
            } else {
                "../slot-retro/tests/fixtures/software.c"
            };
            assert!(std::process::Command::new("cc")
                .args([
                    if cfg!(target_os = "macos") {
                        "-dynamiclib"
                    } else {
                        "-shared"
                    },
                    "-fPIC",
                    "-DSLOT_INPUT_CONTRACT"
                ])
                .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture))
                .arg("-o")
                .arg(root.join("System").join(slot::core::dylib_name(core)))
                .status()
                .unwrap()
                .success());
            let name = if core == Core::SwanStation {
                "advertised.cue"
            } else {
                "input.cue"
            };
            std::fs::write(
                root.join("Games/PS").join(name),
                "FILE \"input.bin\" BINARY\n TRACK 01 MODE2/2352\n INDEX 01 00:00:00\n",
            )
            .unwrap();
            std::fs::write(root.join("Games/PS/input.bin"), [0; 2352]).unwrap();
            slot_store::write_selected_core(root, name, core).unwrap();
            slot_store::write_slot_state(
                root,
                &SlotState {
                    cart: Some(name.into()),
                    cart_platform: Some(Platform::Ps),
                    clock_set: true,
                    ps_dualshock: dualshock,
                    ..SlotState::default()
                },
            )
            .unwrap();
            let mut session = Session::boot_with(root.into(), Box::new(StubSink::draining()));
            let deadline = Instant::now() + Duration::from_secs(5);
            while !matches!(session.app().phase(), Phase::Playing { .. }) {
                session.update(0.02);
                assert!(Instant::now() < deadline, "{core:?} never entered Playing");
                std::thread::sleep(Duration::from_millis(2));
            }
            let controller = match (dualshock, core) {
                (false, _) => 1,
                (true, Core::SwanStation) => 261,
                (true, _) => 517,
            };
            session.set_analog([[-20000, 1200], [14000, -30000]]);
            let sticks = [45536, 1200, 14000, 35536];
            echo(&mut session, controller, 0, sticks);
            let mut now = 1000;
            for (button, mask) in [
                (Btn::L2, ButtonMask::L2),
                (Btn::R2, ButtonMask::R2),
                (Btn::R2, ButtonMask::R2),
                (Btn::A, ButtonMask::A),
                (Btn::B, ButtonMask::B),
                (Btn::X, ButtonMask::X),
                (Btn::Y, ButtonMask::Y),
                (Btn::Up, ButtonMask::UP),
                (Btn::Down, ButtonMask::DOWN),
                (Btn::Left, ButtonMask::LEFT),
                (Btn::Right, ButtonMask::RIGHT),
                (Btn::L1, ButtonMask::L),
                (Btn::R1, ButtonMask::R),
                (Btn::Select, ButtonMask::SELECT),
                (Btn::Start, ButtonMask::START),
            ] {
                session.feed([RawEvent::Down(button)], now);
                session.feed([], now + 650);
                echo(&mut session, controller, mask, sticks);
                session.feed([RawEvent::Up(button)], now + 700);
                echo(&mut session, controller, 0, sticks);
                now += 750;
            }
            for analog in [!dualshock, dualshock] {
                toggle_controller(&mut session);
                session.feed([], now);
                let controller = if !analog {
                    1
                } else if core == Core::SwanStation {
                    261
                } else {
                    517
                };
                echo(&mut session, controller, 0, sticks);
                session.feed(
                    [
                        RawEvent::Down(Btn::L2),
                        RawEvent::Down(Btn::R2),
                        RawEvent::Down(Btn::B),
                    ],
                    now + 1,
                );
                echo(
                    &mut session,
                    controller,
                    ButtonMask::L2 | ButtonMask::R2 | ButtonMask::B,
                    sticks,
                );
                session.feed(
                    [
                        RawEvent::Up(Btn::L2),
                        RawEvent::Up(Btn::R2),
                        RawEvent::Up(Btn::B),
                    ],
                    now + 20,
                );
                echo(&mut session, controller, 0, sticks);
                now += 1000;
            }
        }
    }
}
