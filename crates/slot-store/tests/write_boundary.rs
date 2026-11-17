use slot_store::{atomic_write, fs};

#[test]
fn sd_boundary_rejects_escape_links_and_replaced_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("card");
    std::fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let outside = temp.path().join("outside");
    std::fs::write(&outside, b"unchanged").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        assert!(fs::confine_writes(&root).is_err());
        std::fs::remove_file(root.join("link")).unwrap();
    }
    fs::confine_writes(&root).unwrap();
    fs::create_dir_all(root.join("Saves")).unwrap();
    atomic_write(&root.join("Saves/game.sav"), b"saved").unwrap();
    assert_eq!(
        std::fs::read(root.join("Saves/game.sav")).unwrap(),
        b"saved"
    );
    for path in [outside.clone(), root.join("../outside"), "relative".into()] {
        assert!(fs::write(path, b"bad").is_err());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        assert!(fs::write(root.join("link"), b"bad").is_err());
        std::fs::hard_link(&outside, root.join("hardlink")).unwrap();
        assert!(fs::write(root.join("hardlink"), b"bad").is_err());
    }
    assert!(fs::remove_dir_all(&root).is_err());
    let collection = slot_store::Collection {
        name: "Favorites".into(),
        games: vec![],
    };
    slot_store::write_collection(&root, &collection).unwrap();
    let outside_dir = temp.path().join("outside-dir");
    assert!(slot_store::write_collection(&outside_dir, &collection).is_err());
    assert!(!outside_dir.exists());
    std::fs::rename(&root, temp.path().join("removed-card")).unwrap();
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join("Collections")).unwrap();
    let replacement = root.join("Collections/Favorites.txt");
    std::fs::write(&replacement, b"replacement card").unwrap();
    assert!(slot_store::delete_collection(&root, "Favorites").is_err());
    assert_eq!(std::fs::read(&replacement).unwrap(), b"replacement card");
    assert!(fs::write(root.join("new-file"), b"bad").is_err());
    assert!(!root.join("new-file").exists());
    std::fs::create_dir_all(root.join("Games/GBA")).unwrap();
    let rom = root.join("Games/GBA/replacement.gba");
    std::fs::write(&rom, b"replacement card game").unwrap();
    let cart = slot_store::Cart {
        platform: slot_store::Platform::Gba,
        stem: "replacement".into(),
        rom: rom.clone(),
        label: None,
        title: String::new(),
        tags: Vec::new(),
        code: String::new(),
        shell: None,
    };
    assert!(slot_store::remove_game(&root, &cart).is_err());
    assert_eq!(std::fs::read(rom).unwrap(), b"replacement card game");
    assert_eq!(std::fs::read(outside).unwrap(), b"unchanged");
}
