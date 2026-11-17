use std::collections::HashMap;
use std::path::Path;

use crate::Platform;

pub const SELECTED_CORE_FILE: &str = "Config/selected_core.txt";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Core {
    #[default]
    Mgba,
    Gpsp,
    Gambatte,
    PcsxRearmed,
    Fake08,
    SwanStation,
    #[cfg(feature = "nds")]
    Dsperate,
}

impl Core {
    pub const ALL: [Core; 6 + cfg!(feature = "nds") as usize] = [
        Core::Mgba,
        Core::Gpsp,
        Core::Gambatte,
        Core::PcsxRearmed,
        Core::Fake08,
        Core::SwanStation,
        #[cfg(feature = "nds")]
        Core::Dsperate,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Core::Mgba => "mgba",
            Core::Gpsp => "gpsp",
            Core::Gambatte => "gambatte",
            Core::PcsxRearmed => "pcsx_rearmed",
            Core::Fake08 => "fake08",
            Core::SwanStation => "swanstation",
            #[cfg(feature = "nds")]
            Core::Dsperate => "dsperate",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            Core::Mgba => "mGBA",
            Core::Gpsp => "gpSP",
            Core::Gambatte => "Gambatte",
            Core::PcsxRearmed => "PCSX ReARMed",
            Core::Fake08 => "fake-08",
            Core::SwanStation => "SwanStation",
            #[cfg(feature = "nds")]
            Core::Dsperate => "DSperate",
        }
    }

    pub fn parse(s: &str) -> Option<Core> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mgba" => Some(Core::Mgba),
            "gpsp" => Some(Core::Gpsp),
            "gambatte" => Some(Core::Gambatte),
            "pcsx_rearmed" => Some(Core::PcsxRearmed),
            "fake08" => Some(Core::Fake08),
            "swanstation" => Some(Core::SwanStation),
            #[cfg(feature = "nds")]
            "dsperate" => Some(Core::Dsperate),
            _ => None,
        }
    }

    pub fn runs(self, platform: Platform) -> bool {
        match self {
            #[cfg(feature = "nds")]
            _ if platform == Platform::Nds => self == Core::Dsperate,
            Core::Mgba => !platform.native(),
            Core::Gpsp => platform.hardware() == Platform::Gba,
            Core::Gambatte => matches!(platform, Platform::Gb | Platform::Gbc),
            Core::PcsxRearmed | Core::SwanStation => platform == Platform::Ps,
            Core::Fake08 => platform == Platform::Pico8,
            #[cfg(feature = "nds")]
            Core::Dsperate => false,
        }
    }

    pub fn pair(platform: Platform) -> Option<[Core; 2]> {
        match platform {
            Platform::Gba | Platform::Gbav => Some([Core::Mgba, Core::Gpsp]),
            Platform::Gb | Platform::Gbc => Some([Core::Mgba, Core::Gambatte]),
            Platform::Ps => Some([Core::PcsxRearmed, Core::SwanStation]),
            Platform::Pico8 => None,
            #[cfg(feature = "nds")]
            Platform::Nds => None,
        }
    }
}

pub fn read_selected_cores(root: &Path) -> HashMap<String, Core> {
    crate::ini::read(root, SELECTED_CORE_FILE)
        .into_iter()
        .filter_map(|(stem, name)| Core::parse(&name).map(|core| (stem, core)))
        .collect()
}

pub fn core_for_platform(root: &Path, stem: &str, platform: Platform, gb_core: Core) -> Core {
    let mut lines = crate::ini::read(root, SELECTED_CORE_FILE);
    lines
        .remove(&core_key(platform, stem))
        .or_else(|| lines.remove(stem))
        .as_deref()
        .and_then(Core::parse)
        .filter(|core| core.runs(platform))
        .unwrap_or_else(|| match platform {
            Platform::Gb | Platform::Gbc if gb_core.runs(platform) => gb_core,
            Platform::Ps => Core::PcsxRearmed,
            Platform::Pico8 => Core::Fake08,
            #[cfg(feature = "nds")]
            Platform::Nds => Core::Dsperate,
            _ => Core::Mgba,
        })
}

pub fn write_selected_core(
    root: &Path,
    platform: Platform,
    stem: &str,
    core: Core,
) -> std::io::Result<()> {
    let path = root.join(SELECTED_CORE_FILE);
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let bare = crate::ini::set(&existing, stem, None)?;
    let out = crate::ini::set(&bare, &core_key(platform, stem), Some(core.as_str()))?;
    if let Some(dir) = path.parent() {
        crate::fs::create_dir_all(dir)?;
    }
    crate::atomic::atomic_write(&path, out.as_bytes())
}

fn core_key(platform: Platform, stem: &str) -> String {
    format!("{}/{stem}", platform.dir_name())
}
