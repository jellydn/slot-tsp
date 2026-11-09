use std::io::Write;
use std::path::Path;

use slot::romm::install::{free_space, install, sweep, target_dir, Stopped, TEMP_DIR};
use slot::romm::{
    Api, ApiError, Cancel, Install, Outcome, Pending, Poll, RomFile, RomInfo, RommConf,
};
use slot_store::Platform;
use tempfile::tempdir;

struct Fake {
    files: Vec<RomFile>,
    asked: std::sync::Mutex<Vec<(u64, String, Vec<u64>)>>,
    slug: &'static str,
    name: String,
    bytes: Vec<u8>,
    size: Option<u64>,
    fail: Option<ApiError>,
    bundle: Vec<(u64, Vec<u8>)>,
    fail_on: Option<u64>,
    observe: Option<fn(&Path, &Cancel, &[u64])>,
}

impl Fake {
    fn new(slug: &'static str, name: &str, bytes: Vec<u8>) -> Self {
        Fake {
            slug,
            files: Vec::new(),
            asked: std::sync::Mutex::default(),
            name: name.to_string(),
            bytes,
            size: None,
            fail: None,
            bundle: Vec::new(),
            fail_on: None,
            observe: None,
        }
    }
}

impl Api for Fake {
    fn version(&self) -> Result<String, ApiError> {
        Ok("5.4.0".into())
    }
    fn pair_start(&self, _: &RommConf) -> Result<Pending, ApiError> {
        Err(ApiError::Offline)
    }
    fn pair_poll(&self, _: &str) -> Result<Poll, ApiError> {
        Err(ApiError::Offline)
    }
    fn enable_install(&self, _: &str, _: &str) -> Result<(), ApiError> {
        Ok(())
    }
    fn claim(&self) -> Result<Vec<Install>, ApiError> {
        Ok(Vec::new())
    }
    fn device_exists(&self) -> Result<bool, ApiError> {
        Ok(true)
    }
    fn rom(&self, _: u64) -> Result<RomInfo, ApiError> {
        Ok(RomInfo {
            platform_slug: self.slug.into(),
            fs_name: self.name.clone(),
            files: self.files.clone(),
        })
    }
    fn download(
        &self,
        rom_id: u64,
        name: &str,
        file_ids: &[u64],
        to: &Path,
        cancel: &Cancel,
    ) -> Result<(), ApiError> {
        self.asked
            .lock()
            .unwrap()
            .push((rom_id, name.to_string(), file_ids.to_vec()));
        let bytes = self
            .bundle
            .iter()
            .find(|(id, _)| file_ids == [*id])
            .map_or(self.bytes.as_slice(), |(_, bytes)| bytes.as_slice());
        std::fs::write(to, bytes).unwrap();
        if let Some(size) = self.size {
            std::fs::OpenOptions::new()
                .write(true)
                .open(to)
                .unwrap()
                .set_len(size)
                .unwrap();
        }
        if let Some(observe) = self.observe {
            observe(to, cancel, file_ids);
        }
        if let Some(e) = self
            .fail
            .as_ref()
            .filter(|_| self.fail_on.is_none_or(|id| file_ids == [id]))
        {
            if *e == ApiError::Cancelled {
                cancel.set();
            }
            return Err(e.clone());
        }
        Ok(())
    }
    fn report(&self, _: &str, _: &Outcome) -> Result<(), ApiError> {
        Ok(())
    }
    fn delete_device(&self) -> Result<(), ApiError> {
        Ok(())
    }
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }
    out
}

fn req() -> Install {
    Install {
        id: "r1".into(),
        rom_id: 7,
        file_ids: vec![1],
    }
}

fn roomy(_: &Path) -> u64 {
    u64::MAX
}

fn run(root: &Path, api: &Fake) -> Result<slot::romm::install::Finished, Stopped> {
    slot::root::ensure(root);
    install(root, api, &req(), &Cancel::default(), roomy)
}

fn failed_with(r: Result<slot::romm::install::Finished, Stopped>) -> String {
    match r.unwrap().outcome {
        Outcome::Failed(reason) => reason,
        other => panic!("{other:?}"),
    }
}

fn temps_empty(root: &Path) -> bool {
    std::fs::read_dir(root.join(TEMP_DIR))
        .unwrap()
        .next()
        .is_none()
}

#[test]
fn target_dir_maps_downloads_to_their_playable_shelves() {
    assert_eq!(target_dir("gb").unwrap().platform(), Some(Platform::Gb));
    assert_eq!(target_dir("gbc").unwrap().platform(), Some(Platform::Gbc));
    assert_eq!(target_dir("GBA").unwrap().platform(), Some(Platform::Gba));
    for (slug, folder, platform) in [
        ("psx", "PS", Platform::Ps),
        ("ps", "PS", Platform::Ps),
        ("pico", "PICO8", Platform::Pico8),
        ("pico8", "PICO8", Platform::Pico8),
        ("PICO-8", "PICO8", Platform::Pico8),
    ] {
        let target = target_dir(slug).unwrap();
        assert_eq!(target.dir_name(), folder);
        assert_eq!(target.platform(), Some(platform));
    }
    assert_eq!(target_dir("gbav"), None);
    assert_eq!(target_dir("snes"), None);
    assert_eq!(target_dir("sega-pico"), None);
}

#[test]
fn new_core_downloads_preserve_single_file_content_and_existing_games() {
    let cart = include_bytes!("../../slot-retro/tests/fixtures/persistence.p8");
    for (slug, folder, name, bytes) in [
        (
            "psx",
            "PS",
            "Disc.CHD",
            b"MComprHDauthored header fixture".as_slice(),
        ),
        (
            "ps",
            "PS",
            "Disc.PBP",
            b"\0PBPauthored header fixture".as_slice(),
        ),
        ("pico", "PICO8", "Cart.p8", cart.as_slice()),
    ] {
        let d = tempdir().unwrap();
        let api = Fake::new(slug, name, bytes.to_vec());
        let done = run(d.path(), &api).unwrap();
        assert_eq!(done.outcome, Outcome::Done, "{name}");
        let platform = target_dir(slug).unwrap().platform().unwrap();
        assert_eq!(done.platform, Some(platform));
        assert_eq!(done.stem.as_deref(), Some(name));
        let carts = slot_store::scan_platform(d.path(), platform).unwrap();
        assert_eq!(carts.len(), 1);
        assert_eq!(carts[0].stem, done.stem.unwrap());
        let target = d.path().join("Games").join(folder).join(name);
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        let other = Fake::new(slug, name, b"replacement".to_vec());
        assert_eq!(
            run(d.path(), &other).unwrap().outcome,
            Outcome::AlreadyInstalled
        );
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        assert!(other.asked.lock().unwrap().is_empty());
        assert!(temps_empty(d.path()));
    }
}

#[test]
fn new_core_zips_extract_one_supported_file_and_clean_up_invalid_payloads() {
    for (slug, name, bytes, folder) in [
        ("psx", "Disc.chd", b"MComprHDauthored".as_slice(), "PS"),
        (
            "pico8",
            "Cart.p8",
            include_bytes!("../../slot-retro/tests/fixtures/persistence.p8").as_slice(),
            "PICO8",
        ),
    ] {
        let d = tempdir().unwrap();
        let api = Fake::new(
            slug,
            "Pack.zip",
            zip_of(&[(name, bytes), ("readme.txt", b"notes")]),
        );
        assert_eq!(run(d.path(), &api).unwrap().outcome, Outcome::Done);
        assert_eq!(
            std::fs::read(d.path().join("Games").join(folder).join(name)).unwrap(),
            bytes
        );
        assert!(temps_empty(d.path()));
    }
    for (slug, name) in [
        ("psx", "Error.chd"),
        ("ps", "Error.pbp"),
        ("pico", "Error.p8"),
        ("pico8", "Error.p8.png"),
    ] {
        let d = tempdir().unwrap();
        let api = Fake::new(
            slug,
            "Bad.zip",
            zip_of(&[(name, b"<html>not a game</html>")]),
        );
        assert_eq!(failed_with(run(d.path(), &api)), "Invalid game file");
        assert!(temps_empty(d.path()));
    }
}

#[test]
fn pico8_png_cart_requires_a_complete_cart_sized_png_and_cart_filename() {
    fn image(w: u32, h: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&vec![0; (w * h * 4) as usize])
            .unwrap();
        bytes
    }
    let d = tempdir().unwrap();
    let bytes = image(160, 205);
    assert_eq!(
        run(d.path(), &Fake::new("pico", "Cart.p8.png", bytes.clone()))
            .unwrap()
            .outcome,
        Outcome::Done
    );
    assert_eq!(
        std::fs::read(d.path().join("Games/PICO8/Cart.p8.png")).unwrap(),
        bytes
    );
    // RomM libraries can store PNG-encoded carts with a .p8 filename.
    // fake-08 detects the actual format from the bytes, not the extension.
    let d = tempdir().unwrap();
    assert_eq!(
        run(d.path(), &Fake::new("pico", "Cart.p8", bytes.clone()))
            .unwrap()
            .outcome,
        Outcome::Done
    );
    assert_eq!(
        std::fs::read(d.path().join("Games/PICO8/Cart.p8")).unwrap(),
        bytes
    );
    let d = tempdir().unwrap();
    assert_eq!(
        failed_with(run(
            d.path(),
            &Fake::new("pico", "Broken.p8", bytes[..bytes.len() / 2].to_vec())
        )),
        "Invalid game file"
    );
    assert_eq!(
        failed_with(run(d.path(), &Fake::new("pico", "Art.png", bytes))),
        "Unsupported file type"
    );
    for name in ["Wrong.p8", "Wrong.p8.png"] {
        assert_eq!(
            failed_with(run(d.path(), &Fake::new("pico", name, image(128, 128)))),
            "Invalid game file"
        );
    }
    assert!(temps_empty(d.path()));
}

#[test]
fn ps1_descriptors_and_tracks_are_not_downloaded_in_isolation() {
    for name in ["Disc.cue", "Track.bin", "Game.m3u"] {
        let d = tempdir().unwrap();
        let api = Fake::new("psx", name, b"authored".to_vec());
        assert_eq!(failed_with(run(d.path(), &api)), "Unsupported file type");
        assert!(api.asked.lock().unwrap().is_empty());
    }
}

fn ps_bundle() -> Fake {
    let mut api = Fake::new("psx", "Authored Racing (USA)", Vec::new());
    api.files = [
        (204, "Authored Racing (USA).bin"),
        (206, "Authored Racing (USA).cue"),
        (205, "Authored Racing (USA).m3u"),
    ]
    .into_iter()
    .map(|(id, name)| RomFile {
        id,
        file_name: name.into(),
    })
    .collect();
    let mut sector = vec![0; 2352];
    sector[1..11].fill(255);
    api.bundle = vec![
        (204, sector),
        (206, b"FILE \"Authored Racing (USA).bin\" BINARY\n TRACK 01 MODE2/2352\n INDEX 01 00:00:00\n".to_vec()),
        (205, b"Authored Racing (USA).cue\n".to_vec()),
    ];
    api
}

#[test]
fn ps1_bundle_publishes_one_complete_playable_entry_without_touching_existing_data() {
    use slot::romm::api::LibraryRom;
    let d = tempdir().unwrap();
    let root = d.path();
    slot::root::ensure(root);
    let api = ps_bundle();
    let library = LibraryRom {
        id: 204,
        name: "Authored Racing".into(),
        info: api.rom(204).unwrap(),
    };
    assert!(library.supported());
    std::fs::write(root.join("Games/PS/Existing.chd"), b"existing").unwrap();
    std::fs::write(root.join("Config/untouched"), b"mine").unwrap();
    let result = install(root, &api, &library.install(), &Cancel::default(), roomy).unwrap();
    assert_eq!(result.outcome, Outcome::Done);
    assert_eq!(result.stem.as_deref(), Some("Authored Racing (USA).m3u"));
    let carts = slot_store::scan_platform(root, Platform::Ps).unwrap();
    assert_eq!(carts.len(), 2);
    let game = carts
        .iter()
        .find(|c| c.stem == "Authored Racing (USA).m3u")
        .unwrap();
    let discs = slot_store::disc::contents(&root.join("Games/PS"), &game.rom).unwrap();
    assert_eq!(discs.len(), 1);
    assert_eq!(std::fs::read(&discs[0]).unwrap(), api.bundle[1].1);
    assert_eq!(
        std::fs::read(discs[0].with_extension("bin")).unwrap(),
        api.bundle[0].1
    );
    assert_eq!(
        std::fs::read(root.join("Games/PS/Existing.chd")).unwrap(),
        b"existing"
    );
    assert_eq!(
        std::fs::read(root.join("Config/untouched")).unwrap(),
        b"mine"
    );
    assert!(temps_empty(root));
}

fn bundle_request(api: &Fake) -> Install {
    slot::romm::api::LibraryRom {
        id: 204,
        name: api.name.clone(),
        info: api.rom(204).unwrap(),
    }
    .install()
}

#[test]
fn ps1_bundle_rejects_unsafe_incomplete_or_unplayable_sets_without_publication() {
    for defect in 0..11 {
        let d = tempdir().unwrap();
        let root = d.path();
        slot::root::ensure(root);
        let mut api = ps_bundle();
        match defect {
            0 => {
                api.bundle[1].1 = b"FILE \"../outside.bin\" BINARY\n TRACK 01 MODE2/2352\n".to_vec()
            }
            1 => api.bundle[2].1 = b"/outside.cue\n".to_vec(),
            2 => api.bundle[1].1 = b"FILE \"missing.bin\" BINARY\n TRACK 01 MODE2/2352\n".to_vec(),
            3 => api.files[0].file_name = "../outside.bin".into(),
            4 => api.files[0].file_name = "AUTHORED RACING (USA).CUE".into(),
            5 => api.bundle[0].1 = b"<html>Not a disc</html>".to_vec(),
            6 => api.bundle[1].1 = vec![b'a'; 65537],
            7 => api.bundle[2].1 = b"Authored Racing (USA).m3u\n".to_vec(),
            8 => api.files[0].id = 206,
            9 => {
                api.files.push(RomFile {
                    id: 999,
                    file_name: "Unreferenced.bin".into(),
                });
                api.bundle.push((999, api.bundle[0].1.clone()));
            }
            10 => api.bundle[1].1 = b"FILE \"Authored Racing (USA).cue\" BINARY\n TRACK 01 MODE2/2352\nFILE \"Authored Racing (USA).bin\" BINARY\n TRACK 02 MODE2/2352\n".to_vec(),
            _ => unreachable!(),
        }
        let out = install(root, &api, &bundle_request(&api), &Cancel::default(), roomy).unwrap();
        assert!(
            matches!(out.outcome, Outcome::Failed(_)),
            "defect {defect}: {out:?}"
        );
        assert!(
            slot_store::scan_platform(root, Platform::Ps)
                .unwrap()
                .is_empty(),
            "defect {defect}"
        );
        assert_eq!(std::fs::read_dir(root.join("Games/PS")).unwrap().count(), 0);
        if root.join(TEMP_DIR).exists() {
            assert!(temps_empty(root));
        }
        assert!(!root.join("Cache/romm/outside.bin").exists());
    }
}

#[test]
fn ps1_bundle_cancel_and_partial_network_failure_leave_no_game_or_payload() {
    for error in [
        ApiError::Offline,
        ApiError::Truncated,
        ApiError::Cancelled,
        ApiError::Write("full".into()),
    ] {
        let d = tempdir().unwrap();
        slot::root::ensure(d.path());
        let mut api = ps_bundle();
        api.fail = Some(error.clone());
        api.fail_on = Some(206);
        let out = install(
            d.path(),
            &api,
            &bundle_request(&api),
            &Cancel::default(),
            roomy,
        );
        match error {
            ApiError::Offline => assert_eq!(out, Err(Stopped::Offline)),
            ApiError::Truncated => assert_eq!(out, Err(Stopped::Transient)),
            ApiError::Cancelled => assert_eq!(out, Err(Stopped::Cancelled)),
            _ => assert!(matches!(out.unwrap().outcome, Outcome::Failed(_))),
        }
        assert_eq!(
            std::fs::read_dir(d.path().join("Games/PS"))
                .unwrap()
                .count(),
            0
        );
        assert!(temps_empty(d.path()));
    }
}

#[test]
fn ps1_bundle_rechecks_collisions_and_cancellation_after_staging() {
    for cancel in [false, true] {
        let d = tempdir().unwrap();
        let root = d.path();
        slot::root::ensure(root);
        let mut api = ps_bundle();
        api.observe = Some(if cancel {
            |to, cancel, ids| {
                let root = to.ancestors().nth(4).unwrap();
                assert!(slot_store::scan_platform(root, Platform::Ps)
                    .unwrap()
                    .is_empty());
                if ids == [205] {
                    cancel.set();
                }
            }
        } else {
            |to, _, ids| {
                let root = to.ancestors().nth(4).unwrap();
                assert!(slot_store::scan_platform(root, Platform::Ps)
                    .unwrap()
                    .is_empty());
                if ids == [205] {
                    std::fs::write(root.join("Games/PS/AUTHORED RACING (USA).M3U"), b"mine")
                        .unwrap();
                }
            }
        });
        let out = install(root, &api, &bundle_request(&api), &Cancel::default(), roomy);
        if cancel {
            assert_eq!(out, Err(Stopped::Cancelled));
        } else {
            assert!(matches!(out.unwrap().outcome, Outcome::Failed(_)));
            assert_eq!(
                std::fs::read(root.join("Games/PS/AUTHORED RACING (USA).M3U")).unwrap(),
                b"mine"
            );
        }
        assert!(!root.join("Games/PS/.romm-204").exists());
        if cancel {
            assert_eq!(std::fs::read_dir(root.join("Games/PS")).unwrap().count(), 0);
        } else {
            assert_eq!(std::fs::read_dir(root.join("Games/PS")).unwrap().count(), 1);
        }
        assert!(temps_empty(root));
    }
}

#[test]
fn ps1_cue_bundle_and_existing_bundle_name_are_handled_without_overwrites() {
    let d = tempdir().unwrap();
    let root = d.path();
    slot::root::ensure(root);
    let mut api = ps_bundle();
    api.files.pop();
    api.bundle.pop();
    let request = bundle_request(&api);
    assert_eq!(request.file_ids.len(), 2);
    assert_eq!(
        install(root, &api, &request, &Cancel::default(), roomy)
            .unwrap()
            .outcome,
        Outcome::Done
    );
    let before = std::fs::read(root.join("Games/PS/.romm-204/Authored Racing (USA).bin")).unwrap();
    api.asked.lock().unwrap().clear();
    assert!(matches!(
        install(root, &api, &request, &Cancel::default(), roomy)
            .unwrap()
            .outcome,
        Outcome::Failed(_)
    ));
    assert!(api.asked.lock().unwrap().is_empty());
    assert_eq!(
        std::fs::read(root.join("Games/PS/.romm-204/Authored Racing (USA).bin")).unwrap(),
        before
    );
}

#[test]
fn ps1_bundle_rejects_directory_and_member_symlinks_without_touching_their_targets() {
    use std::os::unix::fs::symlink;
    for member in [false, true] {
        let d = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let root = d.path();
        slot::root::ensure(root);
        std::fs::write(outside.path().join("precious.bin"), b"mine").unwrap();
        std::fs::write(root.join("Config/protected.bin"), b"mine").unwrap();
        let mut api = ps_bundle();
        if member {
            api.observe = Some(|to, _, ids| {
                if ids == [204] {
                    let target = to.ancestors().nth(4).unwrap().join("Config/protected.bin");
                    std::fs::remove_file(to).unwrap();
                    symlink(target, to).unwrap();
                }
            });
        } else {
            std::fs::remove_dir(root.join("Games/PS")).unwrap();
            symlink(outside.path(), root.join("Games/PS")).unwrap();
        }
        assert!(matches!(
            install(root, &api, &bundle_request(&api), &Cancel::default(), roomy)
                .unwrap()
                .outcome,
            Outcome::Failed(_)
        ));
        assert_eq!(
            std::fs::read(outside.path().join("precious.bin")).unwrap(),
            b"mine"
        );
        assert_eq!(
            std::fs::read(root.join("Config/protected.bin")).unwrap(),
            b"mine"
        );
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 1);
        if root.join(TEMP_DIR).exists() {
            assert!(temps_empty(root));
        }
    }
}

#[test]
fn ps1_downloads_enforce_the_fat32_file_size_boundary_without_reading_the_image() {
    for (size, accepted) in [(u32::MAX as u64, true), (u32::MAX as u64 + 1, false)] {
        let d = tempdir().unwrap();
        let mut api = Fake::new("psx", "Large.chd", b"MComprHD".to_vec());
        api.size = Some(size);
        let out = run(d.path(), &api).unwrap();
        let target = d.path().join("Games/PS/Large.chd");
        if accepted {
            assert_eq!(out.outcome, Outcome::Done);
            assert_eq!(std::fs::metadata(target).unwrap().len(), size);
        } else {
            assert_eq!(
                out.outcome,
                Outcome::Failed("File exceeds the FAT32 size limit".into())
            );
            assert!(!target.exists());
        }
        assert!(temps_empty(d.path()));
    }
}

#[test]
fn plain_rom_lands_in_its_folder() {
    let d = tempdir().unwrap();
    let api = Fake::new("gb", "Tetris.gb", b"rom".to_vec());
    let done = run(d.path(), &api).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert_eq!(done.stem.as_deref(), Some("Tetris"));
    assert_eq!(
        std::fs::read(d.path().join("Games/GB/Tetris.gb")).unwrap(),
        b"rom"
    );
    assert!(temps_empty(d.path()));
}

#[test]
fn zip_with_one_rom_installs_under_the_inner_name() {
    let d = tempdir().unwrap();
    let z = zip_of(&[("readme.txt", b"hi"), ("Game.gba", b"gba-bytes")]);
    let api = Fake::new("gba", "Pack.zip", z);
    let done = run(d.path(), &api).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert_eq!(done.stem.as_deref(), Some("Game"));
    assert_eq!(
        std::fs::read(d.path().join("Games/GBA/Game.gba")).unwrap(),
        b"gba-bytes"
    );
    assert!(temps_empty(d.path()));
}

#[test]
fn zip_with_two_roms_fails() {
    let d = tempdir().unwrap();
    let z = zip_of(&[("A.gba", b"a"), ("B.gba", b"b")]);
    let reason = failed_with(run(d.path(), &Fake::new("gba", "Pack.zip", z)));
    assert_eq!(reason, "Zip holds more than one ROM");
    assert!(temps_empty(d.path()));
}

#[test]
fn zip_with_no_rom_fails() {
    let d = tempdir().unwrap();
    let z = zip_of(&[("readme.txt", b"a")]);
    let reason = failed_with(run(d.path(), &Fake::new("gba", "Pack.zip", z)));
    assert_eq!(reason, "Zip holds no playable ROM");
}

#[test]
fn seven_z_fails() {
    let d = tempdir().unwrap();
    let reason = failed_with(run(d.path(), &Fake::new("gba", "Pack.7z", b"x".to_vec())));
    assert_eq!(reason, "Unsupported file type");
}

#[test]
fn other_platform_fails_with_reason() {
    let d = tempdir().unwrap();
    let reason = failed_with(run(
        d.path(),
        &Fake::new("snes", "Mario.sfc", b"x".to_vec()),
    ));
    assert_eq!(reason, "Unsupported RomM platform");
}

#[test]
fn multi_file_rom_fails() {
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    let mut r = req();
    r.file_ids = vec![1, 2];
    let api = Fake::new("gba", "Game.gba", b"x".to_vec());
    let out = install(d.path(), &api, &r, &Cancel::default(), roomy);
    assert_eq!(failed_with(out), "Multi-file ROMs are not supported");
}

#[test]
fn same_name_is_already_installed() {
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    std::fs::write(d.path().join("Games/GBA/Game.gba"), b"mine").unwrap();
    let api = Fake::new("gba", "Game.gba", b"theirs".to_vec());
    let done = run(d.path(), &api).unwrap();
    assert_eq!(done.outcome, Outcome::AlreadyInstalled);
    assert_eq!(
        std::fs::read(d.path().join("Games/GBA/Game.gba")).unwrap(),
        b"mine"
    );

    let z = zip_of(&[("Game.gba", b"theirs")]);
    let done = run(d.path(), &Fake::new("gba", "Pack.zip", z)).unwrap();
    assert_eq!(done.outcome, Outcome::AlreadyInstalled);
    assert_eq!(
        std::fs::read(d.path().join("Games/GBA/Game.gba")).unwrap(),
        b"mine"
    );
    assert!(temps_empty(d.path()));
}

#[test]
fn zip_entry_with_parent_path_stays_in_the_folder() {
    let d = tempdir().unwrap();
    let z = zip_of(&[("../../evil.gba", b"evil")]);
    let done = run(d.path(), &Fake::new("gba", "Pack.zip", z)).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert!(d.path().join("Games/GBA/evil.gba").is_file());
    assert!(!d.path().join("evil.gba").exists());
}

#[test]
fn zip_entry_with_a_trailing_space_is_trimmed() {
    let d = tempdir().unwrap();
    let z = zip_of(&[("Game.gba ", b"x")]);
    let done = run(d.path(), &Fake::new("gba", "Pack.zip", z)).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert!(d.path().join("Games/GBA/Game.gba").is_file());
}

#[test]
fn name_with_a_slash_stays_in_the_folder() {
    let d = tempdir().unwrap();
    let api = Fake::new("gba", "../../evil.gba", b"x".to_vec());
    let done = run(d.path(), &api).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert!(d.path().join("Games/GBA/evil.gba").is_file());
    assert!(!d.path().join("evil.gba").exists());
}

#[test]
fn rename_failure_says_it_could_not_save() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    let games = d.path().join("Games/GBA");
    std::fs::set_permissions(&games, std::fs::Permissions::from_mode(0o555)).unwrap();
    let api = Fake::new("gba", "Game.gba", b"x".to_vec());
    let out = run(d.path(), &api);
    std::fs::set_permissions(&games, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(failed_with(out), "Could not save the game");
    assert!(temps_empty(d.path()));
}

#[test]
fn write_error_fails_at_once_with_no_retry() {
    let d = tempdir().unwrap();
    let mut api = Fake::new("gba", "Game.gba", b"x".to_vec());
    api.fail = Some(ApiError::Write("disk".into()));
    assert_eq!(failed_with(run(d.path(), &api)), "Card is full");
    assert!(temps_empty(d.path()));
}

#[test]
fn errors_map_to_stops() {
    for (err, want) in [
        (ApiError::Offline, Stopped::Offline),
        (ApiError::Server(502), Stopped::Transient),
        (ApiError::Truncated, Stopped::Transient),
        (ApiError::NotFound, Stopped::Lost(ApiError::NotFound)),
        (
            ApiError::Unauthorized,
            Stopped::Lost(ApiError::Unauthorized),
        ),
    ] {
        let d = tempdir().unwrap();
        let mut api = Fake::new("gba", "Game.gba", b"x".to_vec());
        api.fail = Some(err);
        assert_eq!(run(d.path(), &api), Err(want));
        assert!(temps_empty(d.path()));
    }
    let d = tempdir().unwrap();
    let mut api = Fake::new("gba", "Game.gba", b"x".to_vec());
    api.fail = Some(ApiError::Client(403));
    assert!(failed_with(run(d.path(), &api)).contains("403"));
}

#[test]
fn a_folder_rom_with_one_file_installs_under_that_file_name() {
    let d = tempdir().unwrap();
    let mut api = Fake::new("gba", "Folder", b"x".to_vec());
    api.files = vec![
        RomFile {
            id: 1,
            file_name: "Game.gba".into(),
        },
        RomFile {
            id: 2,
            file_name: "Notes.txt".into(),
        },
    ];
    let out = run(d.path(), &api).unwrap();
    assert_eq!(out.outcome, Outcome::Done);
    assert!(d.path().join("Games/GBA/Game.gba").exists());
    assert_eq!(
        *api.asked.lock().unwrap(),
        [(7, "Game.gba".into(), vec![1])]
    );
    api.asked.lock().unwrap().clear();
    let mut selected = req();
    selected.file_ids = vec![99];
    let out = install(d.path(), &api, &selected, &Cancel::default(), roomy);
    assert_eq!(failed_with(out), "Selected file is no longer available");
    assert!(api.asked.lock().unwrap().is_empty());
}

#[test]
fn ps1_selected_file_is_not_replaced_by_the_first_file() {
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    let bytes = b"MComprHDauthored second image";
    let mut api = Fake::new("psx", "Folder", bytes.to_vec());
    api.files = vec![
        RomFile {
            id: 11,
            file_name: "First.chd".into(),
        },
        RomFile {
            id: 22,
            file_name: "Second.chd".into(),
        },
    ];
    let selected = Install {
        id: "selected".into(),
        rom_id: 42,
        file_ids: vec![22],
    };
    let done = install(d.path(), &api, &selected, &Cancel::default(), roomy).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert_eq!(
        *api.asked.lock().unwrap(),
        [(42, "Second.chd".into(), vec![22])]
    );
    assert_eq!(
        std::fs::read(d.path().join("Games/PS/Second.chd")).unwrap(),
        bytes
    );
    assert!(!d.path().join("Games/PS/First.chd").exists());
    assert!(temps_empty(d.path()));
}

#[test]
fn cancel_leaves_no_temp_and_no_rom() {
    let d = tempdir().unwrap();
    let mut api = Fake::new("gba", "Game.gba", b"x".to_vec());
    api.fail = Some(ApiError::Cancelled);
    assert_eq!(run(d.path(), &api), Err(Stopped::Cancelled));
    assert!(temps_empty(d.path()));
    assert!(!d.path().join("Games/GBA/Game.gba").exists());
}

#[test]
fn sweep_clears_temps() {
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    std::fs::write(d.path().join(TEMP_DIR).join("a.part"), b"x").unwrap();
    sweep(d.path());
    assert!(temps_empty(d.path()));
    std::fs::write(d.path().join(TEMP_DIR).join("b.rom"), b"x").unwrap();
    slot::root::ensure(d.path());
    assert!(temps_empty(d.path()));
}

#[test]
fn full_card_fails() {
    fn tight(_: &Path) -> u64 {
        10
    }
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    let z = zip_of(&[("Game.gba", &[7u8; 4096])]);
    let out = install(
        d.path(),
        &Fake::new("gba", "Pack.zip", z),
        &req(),
        &Cancel::default(),
        tight,
    );
    assert_eq!(failed_with(out), "Card is full");
    assert!(temps_empty(d.path()));
    assert!(!d.path().join("Games/GBA/Game.gba").exists());
}

#[test]
fn free_space_reads_the_real_card() {
    let d = tempdir().unwrap();
    assert!(free_space(d.path()) > 0);
}
