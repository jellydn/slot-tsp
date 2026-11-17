mod common;

use common::{clocked, tmp_root_with_carts};
use slot::session::Session;
use slot_input::{Btn, RawEvent};

fn tap(s: &mut Session, now: &mut u64, btn: Btn) {
    *now += 200;
    s.feed([RawEvent::Down(btn)], *now);
    *now += 200;
    s.feed([RawEvent::Up(btn)], *now);
}

#[test]
fn x_requires_explicit_confirmation_then_refreshes_the_library() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut s = Session::boot_with(
        d.path().to_path_buf(),
        Box::new(slot::audio::StubSink::draining()),
    );
    let rom = d.path().join("Games/GBA/Emerald.gba");
    let kept = [
        "Saves/Emerald.sav",
        "States/Emerald.state",
        "Config/game.ini",
        "Config/romm_gifts.txt",
        "Labels/GBA/Emerald.png",
        "Cards/Emerald.mcd",
    ];
    for path in kept {
        let path = d.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"preserve exactly").unwrap();
    }
    let mut now = 0;
    tap(&mut s, &mut now, Btn::X);
    let lines = s.app().game_removal_lines().unwrap();
    assert!(lines.iter().any(|l| l == "Games/GBA/Emerald.gba"));
    assert_eq!(lines.last().unwrap(), "> Cancel <     Delete");
    tap(&mut s, &mut now, Btn::A);
    assert!(rom.exists(), "Cancel must be selected by default");
    tap(&mut s, &mut now, Btn::X);
    tap(&mut s, &mut now, Btn::Right);
    tap(&mut s, &mut now, Btn::A);
    assert!(!rom.exists(), "confirmed selected file was not removed");
    assert_eq!(s.app().selected_stem(), Some("Fusion"));
    assert!(s.app_mut().take_rescanned());
    assert!(d.path().join("Games/GBA/Fusion.gba").exists());
    for path in kept {
        assert_eq!(
            std::fs::read(d.path().join(path)).unwrap(),
            b"preserve exactly"
        );
    }
}

#[test]
fn cancellation_and_failed_removal_keep_the_selection_and_block_other_shelf_actions() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut s = Session::boot_with(
        d.path().to_path_buf(),
        Box::new(slot::audio::StubSink::draining()),
    );
    let mut now = 0;
    let rom = d.path().join("Games/GBA/Emerald.gba");
    let original = std::fs::read(&rom).unwrap();
    tap(&mut s, &mut now, Btn::X);
    tap(&mut s, &mut now, Btn::Y);
    tap(&mut s, &mut now, Btn::R1);
    assert!(!s.app().is_favorite(slot_store::Platform::Gba, "Emerald"));
    tap(&mut s, &mut now, Btn::Right);
    tap(&mut s, &mut now, Btn::B);
    assert!(s.app().game_removal_lines().is_none());
    assert_eq!(std::fs::read(&rom).unwrap(), original);
    assert_eq!(s.app().selected_stem(), Some("Emerald"));
    tap(&mut s, &mut now, Btn::Y);
    assert!(s.app().is_favorite(slot_store::Platform::Gba, "Emerald"));
    tap(&mut s, &mut now, Btn::X);
    std::fs::remove_file(&rom).unwrap();
    std::fs::create_dir(&rom).unwrap();
    tap(&mut s, &mut now, Btn::Right);
    tap(&mut s, &mut now, Btn::A);
    let lines = s.app().game_removal_lines().unwrap();
    assert!(lines.iter().any(|l| l.starts_with("Not deleted:")));
    assert_eq!(lines.last().unwrap(), "> Cancel <     Delete");
    assert!(rom.is_dir());
    assert_eq!(s.app().selected_stem(), Some("Emerald"));
    assert!(!s.app_mut().take_rescanned());
    tap(&mut s, &mut now, Btn::B);
    tap(&mut s, &mut now, Btn::Menu);
    tap(&mut s, &mut now, Btn::X);
    assert!(
        s.app().game_removal_lines().is_none(),
        "X escaped the menu context"
    );
}

#[test]
fn a_new_playlist_blocks_disc_deletion_and_deleting_the_playlist_keeps_its_disc() {
    let d = tmp_root_with_carts(&[]);
    clocked(d.path());
    let ps = d.path().join("Games/PS");
    std::fs::write(ps.join("Disc.chd"), b"disc payload").unwrap();
    std::fs::write(ps.join("Zzz.pbp"), b"other disc").unwrap();
    let mut s = Session::boot_with(
        d.path().to_path_buf(),
        Box::new(slot::audio::StubSink::draining()),
    );
    let mut now = 0;
    tap(&mut s, &mut now, Btn::X);
    std::fs::write(ps.join("Set.m3u"), b"Disc.chd\n").unwrap();
    tap(&mut s, &mut now, Btn::Right);
    tap(&mut s, &mut now, Btn::A);
    assert!(s
        .app()
        .game_removal_lines()
        .unwrap()
        .iter()
        .any(|l| l.contains("Shared or unsafe")));
    assert_eq!(std::fs::read(ps.join("Disc.chd")).unwrap(), b"disc payload");
    drop(s);
    let mut s = Session::boot_with(
        d.path().to_path_buf(),
        Box::new(slot::audio::StubSink::draining()),
    );
    assert_eq!(s.app().selected_stem(), Some("Set.m3u"));
    tap(&mut s, &mut now, Btn::X);
    tap(&mut s, &mut now, Btn::Right);
    tap(&mut s, &mut now, Btn::A);
    assert!(!ps.join("Set.m3u").exists());
    assert_eq!(std::fs::read(ps.join("Disc.chd")).unwrap(), b"disc payload");
    assert_eq!(s.app().selected_stem(), Some("Disc.chd"));
    for name in ["Disc.chd", "Zzz.pbp"] {
        assert_eq!(s.app().selected_stem(), Some(name));
        tap(&mut s, &mut now, Btn::X);
        tap(&mut s, &mut now, Btn::Right);
        tap(&mut s, &mut now, Btn::A);
    }
    assert!(s.app().selected_stem().is_none());
    tap(&mut s, &mut now, Btn::X);
    assert!(s.app().game_removal_lines().is_none());
}
