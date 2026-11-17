use std::io;
use std::path::{Component, Path, PathBuf};

use crate::{Cart, Platform};

fn unsafe_removal(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn selected_file(root: &Path, cart: &Cart) -> io::Result<PathBuf> {
    let relative = cart
        .rom
        .strip_prefix(root)
        .map_err(|_| unsafe_removal("selected game is outside its platform's Games directory"))?;
    let parts: Vec<_> = relative.components().collect();
    if parts.len() != 3
        || parts[0] != Component::Normal("Games".as_ref())
        || parts[1] != Component::Normal(cart.platform.dir_name().as_ref())
        || !matches!(parts[2], Component::Normal(_))
        || !cart.platform.accepts(&cart.rom)
    {
        return Err(unsafe_removal(
            "select a top-level game in its platform's Games directory",
        ));
    }
    for dir in [
        root.to_path_buf(),
        root.join("Games"),
        root.join("Games").join(cart.platform.dir_name()),
    ] {
        if !std::fs::symlink_metadata(&dir)?.file_type().is_dir() {
            return Err(unsafe_removal("game directory must not be a symlink"));
        }
    }
    if !std::fs::symlink_metadata(&cart.rom)?.file_type().is_file() {
        return Err(unsafe_removal(
            "selected game must be a regular file, not a symlink",
        ));
    }
    let path = cart.rom.canonicalize()?;
    if path.parent()
        != Some(
            root.canonicalize()?
                .join("Games")
                .join(cart.platform.dir_name())
                .as_path(),
        )
    {
        return Err(unsafe_removal(
            "selected game resolves outside its Games directory",
        ));
    }
    Ok(path)
}

fn descriptor(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cue") || ext.eq_ignore_ascii_case("m3u"))
}

fn check_references(
    root: &Path,
    dir: &Path,
    selected: &Path,
    budget: &mut usize,
    depth: usize,
) -> io::Result<()> {
    if depth > 32 {
        return Err(unsafe_removal(
            "cannot check disc references: Games/PS is too deep",
        ));
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if *budget == 0 {
            return Err(unsafe_removal(
                "cannot check disc references: Games/PS exceeds 4096 entries",
            ));
        }
        *budget -= 1;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            check_references(root, &path, selected, budget, depth + 1)?;
            continue;
        }
        if kind.is_symlink() {
            let target = path.canonicalize()?;
            if target == selected {
                return Err(unsafe_removal("selected disc is referenced by a symlink"));
            }
            if !std::fs::metadata(&target)?.is_file() || (descriptor(&target) && !descriptor(&path))
            {
                return Err(unsafe_removal(
                    "cannot check disc references through a directory or renamed descriptor symlink",
                ));
            }
        }
        if !descriptor(&path) || path == selected {
            continue;
        }
        if !std::fs::metadata(&path)?.is_file() {
            return Err(unsafe_removal("disc descriptor must be a regular file"));
        }
        let references = crate::disc::references(root, &path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "cannot check disc references in {}: {error}",
                    path.display()
                ),
            )
        })?;
        for reference in references {
            if reference.canonicalize()? == selected {
                return Err(unsafe_removal(
                    "selected disc is referenced by another CUE or M3U",
                ));
            }
        }
    }
    Ok(())
}

/// Check whether only the selected ROM can be removed. Does not change any files.
/// Invalid or unbounded PS descriptor trees fail closed.
pub fn validate_remove_game(root: &Path, cart: &Cart) -> io::Result<()> {
    let selected = selected_file(root, cart)?;
    if cart.platform == Platform::Ps {
        let games = root.join("Games/PS").canonicalize()?;
        check_references(&games, &games, &selected, &mut 4096, 0)?;
    }
    Ok(())
}

/// Recheck the selection and references, then unlink only `cart.rom`.
/// Saves, artwork, configuration and descriptor members are never removed.
pub fn remove_game(root: &Path, cart: &Cart) -> io::Result<()> {
    validate_remove_game(root, cart)?;
    selected_file(root, cart)?;
    crate::fs::remove_file(&cart.rom)
}
