//! A bundle becomes visible only when its top-level playlist is committed.
use std::collections::HashSet;
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::api::{Api, Cancel, DownloadProgress, Install, Outcome, RomInfo};
use super::install::{failed, stopped, Finished, Stopped, TEMP_DIR};

const INVALID: &str = "Invalid or incomplete PS1 disc bundle";
const COLLISION: &str = "A game or bundle with this name already exists";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && name.trim() == name
        && !name.ends_with('.')
        && !name
            .chars()
            .any(|c| c.is_control() || "/\\:<>\"|?*".contains(c))
}

/// Metadata eligibility only; references and content are checked after staging.
pub(super) fn entry(rom: &RomInfo) -> Option<usize> {
    if !matches!(
        rom.platform_slug.to_ascii_lowercase().as_str(),
        "ps" | "psx"
    ) || !(2..=128).contains(&rom.files.len())
    {
        return None;
    }
    let mut names = HashSet::new();
    let mut ids = HashSet::new();
    let mut cues = Vec::new();
    let mut playlists = Vec::new();
    for (i, file) in rom.files.iter().enumerate() {
        if !safe_name(&file.file_name)
            || !names.insert(file.file_name.to_lowercase())
            || !ids.insert(file.id)
        {
            return None;
        }
        match extension(&file.file_name).as_str() {
            "bin" => (),
            "cue" => cues.push(i),
            "m3u" => playlists.push(i),
            _ => return None,
        }
    }
    if cues.is_empty() {
        return None;
    }
    match playlists.as_slice() {
        [playlist] => Some(*playlist),
        [] if cues.len() == 1 => Some(cues[0]),
        _ => None,
    }
}

struct Pending(Option<PathBuf>);
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = slot_store::fs::remove_dir_all(path);
        }
    }
}

// Normal rename can replace an existing file (or empty directory). Never do so,
// including if another installer creates the destination after preflight.
fn publish(from: &Path, to: &Path) -> io::Result<()> {
    slot_store::fs::check_write(from)?;
    slot_store::fs::check_write(to)?;
    let from = std::ffi::CString::new(from.as_os_str().as_encoded_bytes())?;
    let to = std::ffi::CString::new(to.as_os_str().as_encoded_bytes())?;
    #[cfg(target_os = "macos")]
    let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn directory(path: &Path) -> io::Result<()> {
    slot_store::fs::check_write(path)?;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_dir() => Ok(()),
        Ok(_) => Err(io::Error::other("bundle directory is not a real directory")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => std::fs::create_dir(path),
        Err(e) => Err(e),
    }
}

fn validate(stage: &Path, rom: &RomInfo, entry: usize) -> io::Result<Vec<PathBuf>> {
    let selected = stage.join(&rom.files[entry].file_name);
    let mut closure: HashSet<_> = slot_store::disc::references(stage, &selected)?
        .into_iter()
        .collect();
    closure.insert(selected.clone());
    let manifest: HashSet<_> = rom
        .files
        .iter()
        .map(|file| stage.join(&file.file_name))
        .collect();
    if closure != manifest {
        return Err(io::Error::other(INVALID));
    }
    let mut identities = HashSet::new();
    for file in &rom.files {
        let path = stage.join(&file.file_name);
        let meta = std::fs::symlink_metadata(&path)?;
        if !meta.file_type().is_file()
            || meta.nlink() != 1
            || meta.len() == 0
            || meta.len() > u32::MAX as u64
            || !identities.insert((meta.dev(), meta.ino()))
        {
            return Err(io::Error::other(INVALID));
        }
        if extension(&file.file_name) == "bin" {
            let mut sync = [0; 12];
            std::fs::File::open(&path)?.read_exact(&mut sync)?;
            if meta.len() % 2352 != 0
                || sync != [0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0]
            {
                return Err(io::Error::other(
                    "bundle requires raw 2352-byte data tracks",
                ));
            }
        }
        if extension(&file.file_name) == "cue" {
            if slot_store::disc::references(stage, &path)?
                .iter()
                .any(|track| {
                    !track
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("bin"))
                })
            {
                return Err(io::Error::other(INVALID));
            }
            // The shared reference reader has already imposed UTF-8/64-KiB and
            // core parser line limits. Do not accept other FILE container types.
            let text = std::fs::read_to_string(&path)?;
            let mut data = false;
            for line in text.lines() {
                let words: Vec<_> = line.split_whitespace().collect();
                match words.first().map(|w| w.to_ascii_uppercase()).as_deref() {
                    Some("FILE") if !line.trim_end().to_ascii_uppercase().ends_with(" BINARY") => {
                        return Err(io::Error::other(INVALID));
                    }
                    Some("TRACK") => {
                        if words.len() != 3
                            || !matches!(
                                words[2].to_ascii_uppercase().as_str(),
                                "MODE1/2352" | "MODE2/2352" | "AUDIO"
                            )
                        {
                            return Err(io::Error::other(INVALID));
                        }
                        data |= !words[2].eq_ignore_ascii_case("AUDIO");
                    }
                    _ => (),
                }
            }
            if !data {
                return Err(io::Error::other(INVALID));
            }
        }
    }
    slot_store::disc::contents(stage, &selected)
}

pub(super) fn install(
    root: &Path,
    api: &dyn Api,
    req: &Install,
    cancel: &Cancel,
    rom: &RomInfo,
    entry: usize,
) -> Result<Finished, Stopped> {
    let requested: HashSet<_> = req.file_ids.iter().copied().collect();
    let all: HashSet<_> = rom.files.iter().map(|f| f.id).collect();
    if !requested.is_empty()
        && requested != all
        && requested != HashSet::from([rom.files[entry].id])
    {
        return failed("Select the complete PS1 disc bundle");
    }
    let name = &rom.files[entry].file_name;
    let stem = Path::new(name).file_stem().unwrap().to_str().unwrap();
    let games = root.join("Games/PS");
    for path in [
        root.to_path_buf(),
        root.join("Games"),
        games.clone(),
        root.join("Cache"),
        root.join(TEMP_DIR),
    ] {
        if directory(&path).is_err() {
            return failed("Could not create safe bundle directories");
        }
    }
    let target = games.join(format!("{stem}.m3u"));
    let payload_name = format!(".romm-{}", req.rom_id);
    let payload = games.join(&payload_name);
    let collision = || -> io::Result<bool> {
        for e in std::fs::read_dir(&games)? {
            let path = e?.path();
            if path
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case(&payload_name))
                || path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case(stem))
            {
                return Ok(true);
            }
        }
        Ok(false)
    };
    if !matches!(collision(), Ok(false)) {
        return failed(COLLISION);
    }
    let stage = root.join(TEMP_DIR).join(format!(
        "bundle-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if slot_store::fs::check_write(&stage)
        .and_then(|_| std::fs::create_dir(&stage))
        .is_err()
    {
        return failed("Could not stage PS1 bundle");
    }
    let mut pending = Pending(Some(stage.clone()));
    for file in &rom.files {
        if cancel.is_set() {
            return Err(Stopped::Cancelled);
        }
        let path = stage.join(&file.file_name);
        if let Err(e) = api.download(req.rom_id, &file.file_name, &[file.id], &path, cancel) {
            return stopped(e);
        }
        if std::fs::File::open(&path)
            .and_then(|f| f.sync_all())
            .is_err()
        {
            return failed("Could not save the game");
        }
    }
    if cancel.is_set() {
        return Err(Stopped::Cancelled);
    }
    cancel.set_progress(DownloadProgress::Installing);
    let discs = match validate(&stage, rom, entry) {
        Ok(discs) => discs,
        Err(_) => return failed(INVALID),
    };
    let playlist: String = discs
        .iter()
        .map(|p| {
            format!(
                "{payload_name}/{}\n",
                p.file_name().unwrap().to_str().unwrap()
            )
        })
        .collect();
    let staged_entry = stage.join(".entry.m3u");
    if slot_store::fs::write(&staged_entry, playlist)
        .and_then(|_| std::fs::File::open(&staged_entry)?.sync_all())
        .is_err()
    {
        return failed("Could not save the game");
    }
    if cancel.is_set() {
        return Err(Stopped::Cancelled);
    }
    if !matches!(collision(), Ok(false)) {
        return failed(COLLISION);
    }
    if publish(&stage, &payload).is_err() {
        return failed("Could not publish PS1 bundle");
    }
    pending.0 = Some(payload.clone());
    if std::fs::File::open(&payload)
        .and_then(|f| f.sync_all())
        .and_then(|_| std::fs::File::open(&games)?.sync_all())
        .is_err()
    {
        return failed("Could not save PS1 bundle directory");
    }
    if cancel.is_set() {
        return Err(Stopped::Cancelled);
    }
    if publish(&payload.join(".entry.m3u"), &target).is_err() {
        return failed("Could not publish PS1 playlist");
    }
    // This single no-replace rename is the library-visible commit point.
    pending.0 = None;
    // Keep the committed payload even if durability confirmation fails: the
    // visible playlist already references it and must not become dangling.
    if std::fs::File::open(&payload)
        .and_then(|f| f.sync_all())
        .and_then(|_| std::fs::File::open(&games)?.sync_all())
        .is_err()
    {
        return failed(
            "PS1 bundle published; storage sync failed. Check the library before retrying",
        );
    }
    Ok(Finished {
        outcome: Outcome::Done,
        stem: Some(format!("{stem}.m3u")),
        platform: Some(slot_store::Platform::Ps),
    })
}
