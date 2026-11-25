//! `victory settings`: the environment switches meant for users, and which
//! of them are set now. Developer switches used while working on one part
//! are left out; this list is the ones the READMEs describe.

use std::fmt::Write as _;

/// Which build reads a switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Build {
    Any,
    /// `cargo build --profile diag -p victory --features diagnostics`.
    Diagnostics,
}

pub(crate) struct Setting {
    pub name: &'static str,
    pub group: &'static str,
    pub build: Build,
    pub about: &'static str,
}

const fn any(group: &'static str, name: &'static str, about: &'static str) -> Setting {
    Setting {
        name,
        group,
        build: Build::Any,
        about,
    }
}

const fn diag(group: &'static str, name: &'static str, about: &'static str) -> Setting {
    Setting {
        name,
        group,
        build: Build::Diagnostics,
        about,
    }
}

const PATHS: &str = "Paths";
const INPUT: &str = "Input";
const SPEED: &str = "Speed (optional, off by default)";
const DEBUG: &str = "Debugging";

#[rustfmt::skip]
pub(crate) const SETTINGS: &[Setting] = &[
    any(PATHS, "VICTORY_CONFIG", "config file to read instead of .\\victory.toml"),
    any(PATHS, "VICTORY_GAME_DIR", "your own game folder ([paths].game_dir)"),
    any(PATHS, "VICTORY_CACHE_DIR", "derived data such as the shader cache ([paths].cache_dir)"),
    any(PATHS, "VICTORY_DATA_DIR", "saves, settings and crashes.log ([paths].data_dir)"),
    any(PATHS, "VICTORY_TEST_TITLES", "other titles for testing, separated by ';'"),
    any(INPUT, "VICTORY_PAD_MAP", "keyboard keys for pad buttons, <key>=<button>,... ([input].pad_map)"),
    diag(INPUT, "VICTORY_KEYS", "press keys at those seconds, <seconds>:<key>,..."),
    any(SPEED, "VICTORY_RELAY", "1 or 2: copy GPU results back in stages (2 cut frame time on test machines)"),
    any(SPEED, "VICTORY_LATE_WAIT", "0 off, or a stage: wait for the GPU later in the frame"),
    any(SPEED, "VICTORY_WIDE_BACK", "1: compare what shaders wrote on several threads"),
    any(SPEED, "VICTORY_PIPELINE_WAIT_MS", "milliseconds per frame to wait for pipelines still being built"),
    any(DEBUG, "VICTORY_LOG", "log filter, e.g. victory_loader=trace,info (release builds stop at info)"),
    diag(DEBUG, "VICTORY_STACKS", "every thread's stack at those seconds, e.g. 20,60"),
    diag(DEBUG, "VICTORY_WATCH", "log a title u32 when it changes, <rva>,... (hex)"),
    diag(DEBUG, "VICTORY_CRASH_SAVE", "save title memory on a crash, <rva>:<len>,... (hex)"),
    diag(DEBUG, "VICTORY_CAPTURE", "folder to save presented frames to as PNG"),
    diag(DEBUG, "VICTORY_CAPTURE_FRAMES", "which frames VICTORY_CAPTURE saves, e.g. 100,200"),
    diag(DEBUG, "VICTORY_DUMP_TARGETS", "1: with a capture, also save every render target"),
    diag(DEBUG, "VICTORY_D3D12_DEBUG", "1: the host D3D12 debug layer"),
    diag(DEBUG, "VICTORY_D3D12_GBV", "1: with VICTORY_D3D12_DEBUG, GPU-based validation (slow)"),
    diag(DEBUG, "VICTORY_D3D12_DRED", "1: on a GPU failure, report the commands in flight"),
    diag(DEBUG, "VICTORY_D3D12_WARP", "1: render on the software adapter (slow; cannot reset the driver)"),
    diag(DEBUG, "VICTORY_SKIP_PIPELINES", "leave out draws whose pipeline has one of these 16-digit shader hashes"),
    diag(DEBUG, "VICTORY_GPU_TIMING", "1: time each command list and log the slow ones"),
    diag(DEBUG, "VICTORY_SHADER_LOOP_LIMIT", "stop translated shader loops after this many iterations"),
    diag(DEBUG, "VICTORY_AUDIO_CAPTURE", "record what the title plays to this .wav file"),
    diag(DEBUG, "VICTORY_XMA_DUMP", "folder to save the first XMA voices' input to"),
];

/// The settings table: each switch with its value now (from `env`), the
/// build it needs, and a note when this build ignores a switch that is set.
pub(crate) fn render(env: impl Fn(&str) -> Option<String>, diagnostics: bool) -> String {
    let mut out = String::new();
    let mut group = "";
    for s in SETTINGS {
        if s.group != group {
            group = s.group;
            let _ = writeln!(out, "\n{group}");
        }
        let value = env(s.name).filter(|v| !v.is_empty());
        let shown = value
            .as_deref()
            .map_or("(not set)".to_owned(), |v| format!("= {v}"));
        let build = match s.build {
            Build::Any => "",
            Build::Diagnostics => "  [diagnostics build]",
        };
        let _ = writeln!(out, "  {:<26}{shown}{build}", s.name);
        let _ = writeln!(out, "      {}", s.about);
        if value.is_some() && s.build == Build::Diagnostics && !diagnostics {
            let _ = writeln!(out, "      ! set, but this build ignores it");
        }
    }
    out
}

pub fn run() {
    print!(
        "{}",
        render(|k| std::env::var(k).ok(), cfg!(feature = "diagnostics"))
    );
    println!("\nSet one in PowerShell with  $env:VICTORY_PAD_MAP = \"Space=A\"");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn every_listed_setting_is_read_by_the_code() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = Vec::new();
        sources(&root.join("crates"), &mut files);
        sources(&root.join("apps"), &mut files);
        let text: String = files
            .iter()
            .filter(|p| !p.ends_with("settings.rs"))
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect();
        let unread: Vec<_> = SETTINGS
            .iter()
            .map(|s| s.name)
            .filter(|name| !text.contains(&format!("\"{name}\"")))
            .collect();
        assert!(unread.is_empty(), "listed but never read: {unread:?}");
    }

    #[test]
    fn names_are_unique_and_grouped() {
        let mut names: Vec<_> = SETTINGS.iter().map(|s| s.name).collect();
        assert!(names.iter().all(|n| n.starts_with("VICTORY_")));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SETTINGS.len(), "a name is listed twice");
        // Each group's settings are together, so each heading prints once.
        let mut seen: Vec<&str> = Vec::new();
        for s in SETTINGS {
            if seen.last() != Some(&s.group) {
                assert!(!seen.contains(&s.group), "{} is split", s.group);
                seen.push(s.group);
            }
        }
    }

    #[test]
    fn the_table_shows_values_and_ignored_switches() {
        let env = |k: &str| match k {
            "VICTORY_PAD_MAP" => Some("Space=A".to_owned()),
            "VICTORY_D3D12_DRED" => Some("1".to_owned()),
            "VICTORY_GAME_DIR" => Some(String::new()),
            _ => None,
        };
        let release = render(env, false);
        assert!(release.contains("VICTORY_PAD_MAP           = Space=A\n"));
        // An empty value counts as not set.
        assert!(release.contains("VICTORY_GAME_DIR          (not set)\n"));
        let dred = release.find("VICTORY_D3D12_DRED").unwrap();
        assert!(release[dred..].starts_with("VICTORY_D3D12_DRED        = 1  [diagnostics build]"));
        assert_eq!(release.matches("this build ignores it").count(), 1);
        // A diagnostics build reads it, so there is nothing to warn about.
        assert!(!render(env, true).contains("ignores"));
    }
}
