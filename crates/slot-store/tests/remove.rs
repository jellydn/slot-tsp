use slot_store::{remove_game, validate_remove_game, Cart, Platform};
use std::{fs, path::Path};

fn write(root: &Path, name: &str, data: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

fn cart(root: &Path, name: &str, platform: Platform) -> Cart {
    Cart {
        platform,
        stem: name.into(),
        rom: root.join(name),
        label: None,
        title: String::new(),
        code: String::new(),
        shell: None,
    }
}

#[test]
fn removes_only_selected_file_and_validation_does_not_write() {
    let root = tempfile::tempdir().unwrap();
    let files = [
        "Games/GBA/game.gba",
        "Games/GBA/other.gba",
        "Saves/game.sav",
        "States/game.state",
        "Config/game.ini",
        "Labels/GBA/game.png",
        "Cards/game.mcd",
        "System/slot.ini",
    ];
    for file in files {
        write(root.path(), file, "keep");
    }
    let selected = cart(root.path(), files[0], Platform::Gba);
    validate_remove_game(root.path(), &selected).unwrap();
    assert!(selected.rom.exists());
    remove_game(root.path(), &selected).unwrap();
    assert!(!selected.rom.exists());
    for file in &files[1..] {
        assert_eq!(fs::read_to_string(root.path().join(file)).unwrap(), "keep");
    }
}

#[test]
fn descriptors_delete_only_the_selected_descriptor() {
    for name in ["set.m3u", "disc.cue"] {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            "Games/PS/disc.cue",
            "FILE \"track.bin\" BINARY\n",
        );
        write(root.path(), "Games/PS/track.bin", "track");
        if name == "set.m3u" {
            write(root.path(), "Games/PS/set.m3u", "disc.cue\n");
        }
        remove_game(
            root.path(),
            &cart(root.path(), &format!("Games/PS/{name}"), Platform::Ps),
        )
        .unwrap();
        assert!(root.path().join("Games/PS/track.bin").exists());
        assert_eq!(
            root.path().join("Games/PS/disc.cue").exists(),
            name == "set.m3u"
        );
    }
}

#[test]
fn scanned_single_file_games_can_be_removed_on_every_platform() {
    for (platform, name) in [
        (Platform::Gba, "GBA/game.gba"),
        (Platform::Gb, "GB/game.gb"),
        (Platform::Gbc, "GBC/game.gbc"),
        (Platform::Gbav, "GBAV/game.gba"),
        (Platform::Ps, "PS/game.chd"),
        (Platform::Ps, "PS/game.pbp"),
        (Platform::Pico8, "PICO8/game.p8.png"),
    ] {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), &format!("Games/{name}"), "rom");
        let selected = slot_store::scan_platform(root.path(), platform)
            .unwrap()
            .remove(0);
        remove_game(root.path(), &selected).unwrap();
        assert!(!selected.rom.exists());
    }
}

#[test]
fn refuses_members_referenced_by_another_playlist() {
    for name in ["disc.chd", "disc.pbp", "disc.cue"] {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            &format!("Games/PS/{name}"),
            if name.ends_with("cue") {
                "FILE track.bin BINARY\n"
            } else {
                "disc"
            },
        );
        write(root.path(), "Games/PS/track.bin", "track");
        write(root.path(), "Games/PS/list.m3u", &format!("{name}\n"));
        let selected = cart(root.path(), &format!("Games/PS/{name}"), Platform::Ps);
        assert!(remove_game(root.path(), &selected)
            .unwrap_err()
            .to_string()
            .contains("referenced"));
        assert!(selected.rom.exists());
    }
}

#[cfg(unix)]
#[test]
fn refuses_a_disc_with_a_standalone_symlink_alias() {
    for name in ["disc.chd", "set.m3u"] {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), "Games/PS/disc.chd", "disc");
        write(root.path(), "Games/PS/set.m3u", "disc.chd\n");
        if name.ends_with("chd") {
            fs::remove_file(root.path().join("Games/PS/set.m3u")).unwrap();
        }
        let alias = root.path().join("Games/PS").join(format!("alias-{name}"));
        std::os::unix::fs::symlink(name, &alias).unwrap();
        let original = fs::read(&alias).unwrap();
        let selected = cart(root.path(), &format!("Games/PS/{name}"), Platform::Ps);
        assert!(remove_game(root.path(), &selected).is_err());
        assert_eq!(fs::read(&alias).unwrap(), original);
    }
}

#[cfg(unix)]
#[test]
fn refuses_shared_symlink_aliases_in_m3u_and_cue() {
    for descriptor in ["list.m3u", "other.cue"] {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), "Games/PS/disc.chd", "disc");
        // Use a nested alias whose canonical target is the selected file.
        fs::create_dir(root.path().join("Games/PS/nested")).unwrap();
        std::os::unix::fs::symlink("../disc.chd", root.path().join("Games/PS/nested/alias.chd"))
            .unwrap();
        write(
            root.path(),
            &format!("Games/PS/nested/{descriptor}"),
            if descriptor.ends_with("m3u") {
                "alias.chd\n"
            } else {
                "FILE alias.chd BINARY\n"
            },
        );
        let selected = cart(root.path(), "Games/PS/disc.chd", Platform::Ps);
        let error = remove_game(root.path(), &selected).unwrap_err();
        assert!(error.to_string().contains("referenced"), "{error}");
        assert!(selected.rom.exists());
    }
}

#[test]
fn rejects_invalid_outside_nested_directory_and_missing_paths() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "Games/GBA/game.gba", "rom");
    write(root.path(), "outside.gba", "rom");
    write(root.path(), "Games/GBA/nested/game.gba", "rom");
    fs::create_dir(root.path().join("Games/GBA/directory.gba")).unwrap();
    for name in [
        "outside.gba",
        "Games/GBA/nested/game.gba",
        "Games/GBA/directory.gba",
        "Games/GBA/missing.gba",
        "Games/GBA/../GBA/game.gba",
    ] {
        assert!(
            remove_game(root.path(), &cart(root.path(), name, Platform::Gba)).is_err(),
            "{name}"
        );
    }
    assert!(remove_game(
        root.path(),
        &cart(root.path(), "Games/GBA/game.gba", Platform::Ps)
    )
    .is_err());
    assert!(root.path().join("Games/GBA/game.gba").exists());
}

#[cfg(unix)]
#[test]
fn rejects_symlink_selection_and_platform_directory() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "Games/GBA/game.gba", "rom");
    std::os::unix::fs::symlink("game.gba", root.path().join("Games/GBA/link.gba")).unwrap();
    assert!(remove_game(
        root.path(),
        &cart(root.path(), "Games/GBA/link.gba", Platform::Gba)
    )
    .is_err());
    fs::rename(root.path().join("Games/GBA"), root.path().join("real")).unwrap();
    std::os::unix::fs::symlink("../real", root.path().join("Games/GBA")).unwrap();
    assert!(remove_game(
        root.path(),
        &cart(root.path(), "Games/GBA/game.gba", Platform::Gba)
    )
    .is_err());
}

#[test]
fn revalidates_after_confirmation_and_propagates_io_failure() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "Games/PS/disc.chd", "disc");
    let selected = cart(root.path(), "Games/PS/disc.chd", Platform::Ps);
    validate_remove_game(root.path(), &selected).unwrap();
    write(root.path(), "Games/PS/new.m3u", "disc.chd\n");
    assert!(remove_game(root.path(), &selected).is_err());
    fs::remove_file(root.path().join("Games/PS/new.m3u")).unwrap();
    fs::remove_file(&selected.rom).unwrap();
    assert_eq!(
        remove_game(root.path(), &selected).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
}

#[test]
fn fails_closed_on_invalid_or_oversized_descriptors() {
    for descriptor in [
        "missing.chd\n".to_string(),
        "../disc.chd\n".to_string(),
        "x".repeat(65537),
    ] {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), "Games/PS/disc.chd", "disc");
        write(root.path(), "Games/PS/bad.m3u", &descriptor);
        let selected = cart(root.path(), "Games/PS/disc.chd", Platform::Ps);
        assert!(remove_game(root.path(), &selected).is_err());
        assert!(selected.rom.exists());
    }
}

#[cfg(unix)]
#[test]
fn propagates_unlink_permission_failure_without_removing_anything() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "Games/GBA/game.gba", "rom");
    let selected = cart(root.path(), "Games/GBA/game.gba", Platform::Gba);
    let dir = root.path().join("Games/GBA");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    let result = remove_game(root.path(), &selected);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        result.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(fs::read_to_string(selected.rom).unwrap(), "rom");
}

#[test]
fn fails_closed_when_reference_traversal_is_too_deep() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "Games/PS/disc.chd", "disc");
    let mut dir = root.path().join("Games/PS");
    for _ in 0..33 {
        dir.push("nested");
    }
    fs::create_dir_all(dir).unwrap();
    let selected = cart(root.path(), "Games/PS/disc.chd", Platform::Ps);
    assert!(remove_game(root.path(), &selected)
        .unwrap_err()
        .to_string()
        .contains("too deep"));
    assert!(selected.rom.exists());
}

#[cfg(unix)]
#[test]
fn fails_closed_on_symlink_directories_and_descriptor_io_errors() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "Games/PS/disc.chd", "disc");
    let selected = cart(root.path(), "Games/PS/disc.chd", Platform::Ps);
    let alias = root.path().join("Games/PS/alias");
    std::os::unix::fs::symlink(".", &alias).unwrap();
    assert!(remove_game(root.path(), &selected).is_err());
    fs::remove_file(alias).unwrap();
    std::os::unix::fs::symlink("missing.m3u", root.path().join("Games/PS/broken.m3u")).unwrap();
    assert_eq!(
        remove_game(root.path(), &selected).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(selected.rom.exists());
}
