mod common;

use std::path::Path;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::App;
use slot_input::{Action, Btn};
use slot_store::{read_slot_state, write_slot_state, SlotState};
use slot_ui::{QuickRow, QuickValue};

fn boot(root: &Path, stick: bool) -> App {
    write_slot_state(
        root,
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .expect("write slot.state");
    let (mut a, _) = app_booting_at(root, CLOCK_IS_SET);
    a.set_left_stick(stick);
    a
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

#[test]
fn a_device_without_a_stick_hides_the_row_and_never_steers() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let a = boot(d.path(), false);
    assert!(a.hidden_rows().contains(&QuickRow::LeftStick));
    assert!(a.in_game_hidden().contains(&QuickRow::LeftStick));
    assert!(!a.left_stick_dpad());
}

#[test]
fn the_stick_is_turned_off_from_gameplay_and_saved() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path(), true);
    assert!(a.left_stick_dpad());
    a.apply(Action::QuickMenu);
    for _ in 0..QuickRow::Game.position(&[]) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    for _ in 0..QuickRow::LeftStick.position(&a.hidden_rows()) {
        press(&mut a, Btn::Down);
    }
    assert_eq!(a.quick_menu(), Some(QuickRow::LeftStick));
    assert_eq!(a.quick_value(QuickRow::LeftStick), Some(QuickValue::On));
    press(&mut a, Btn::Right);
    assert_eq!(a.quick_value(QuickRow::LeftStick), Some(QuickValue::Off));
    assert!(!a.left_stick_dpad());
    assert!(
        !read_slot_state(d.path()).left_stick_dpad,
        "the choice was not saved"
    );
}

#[test]
fn ps1_digital_uses_the_saved_stick_choice_but_dualshock_does_not() {
    for dualshock in [false, true] {
        for enabled in [false, true] {
            let d = tmp_root_with_carts(&[]);
            std::fs::create_dir_all(d.path().join("Games/PS")).unwrap();
            std::fs::write(d.path().join("Games/PS/Pad.chd"), b"authored").unwrap();
            std::fs::write(d.path().join("Games/PS/Zzz.chd"), b"authored").unwrap();
            write_slot_state(
                d.path(),
                &SlotState {
                    clock_set: true,
                    cart_platform: Some(slot_store::Platform::Ps),
                    ps_dualshock: dualshock,
                    left_stick_dpad: enabled,
                    ..SlotState::default()
                },
            )
            .unwrap();
            let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
            a.set_platform(slot_store::Platform::Ps);
            a.set_left_stick(true);
            assert_eq!(a.left_stick_dpad(), enabled && !dualshock);
            assert_eq!(a.hidden_rows().contains(&QuickRow::LeftStick), dualshock);
            a.set_left_stick(false);
            assert!(!a.left_stick_dpad());
            assert!(a.hidden_rows().contains(&QuickRow::LeftStick));
            a.set_left_stick(true);
            a.apply(Action::QuickMenu);
            for _ in 0..QuickRow::Controls.position(&a.hidden_rows()) {
                press(&mut a, Btn::Down);
            }
            press(&mut a, Btn::A);
            for _ in 0..QuickRow::InputTest.position(&a.hidden_rows()) {
                press(&mut a, Btn::Down);
            }
            assert_eq!(a.quick_menu(), Some(QuickRow::InputTest));
            press(&mut a, Btn::A);
            assert!(a.input_test().is_some());
            assert!(!a.left_stick_dpad());
        }
    }
}
