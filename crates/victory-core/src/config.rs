//! Where the user's files live.
//!
//! Resolution order for each value: environment variable, then the config
//! file (`VICTORY_CONFIG`, else `./victory.toml`), then the default.
//! See `victory.example.toml` at the repository root.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::policy::ensure_outside_repo;

#[derive(Debug, Default, Clone)]
pub struct Config {
    /// The user's dump of the title.
    pub game_dir: Option<PathBuf>,
    /// Root for all derived data. Always outside the repository.
    pub cache_dir: PathBuf,
    /// What titles write and must keep (persistent local storage). Never
    /// inside the cache, which may be deleted at any time.
    pub data_dir: PathBuf,
    /// Other titles used to exercise platform-level code.
    pub test_titles: Vec<PathBuf>,
    /// The keyboard's stand-in pad buttons, `<key>=<button>,...`.
    pub pad_map: Option<String>,
    /// The config file that was read, if any.
    pub source: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    paths: FilePaths,
    #[serde(default)]
    input: FileInput,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileInput {
    pad_map: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePaths {
    game_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    #[serde(default)]
    test_titles: Vec<PathBuf>,
}

impl Config {
    /// Loads configuration from the environment and the config file.
    pub fn load() -> Result<Self> {
        let explicit = std::env::var_os("VICTORY_CONFIG").map(PathBuf::from);
        let path = explicit
            .clone()
            .unwrap_or_else(|| PathBuf::from("victory.toml"));
        let file = if path.is_file() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            Some((path, text))
        } else if let Some(p) = explicit {
            anyhow::bail!(
                "VICTORY_CONFIG points at {}, which does not exist",
                p.display()
            );
        } else {
            None
        };
        let (source, parsed) = match file {
            Some((path, text)) => {
                let parsed: FileConfig =
                    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
                (Some(path), parsed)
            }
            None => (None, FileConfig::default()),
        };
        let env = |k: &str| std::env::var_os(k);
        let mut config = Self::resolve(parsed.paths, source, env)?;
        config.pad_map = pad_map(parsed.input, env);
        Ok(config)
    }

    fn resolve(
        file: FilePaths,
        source: Option<PathBuf>,
        env: impl Fn(&str) -> Option<std::ffi::OsString>,
    ) -> Result<Self> {
        let env_path = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        let test_titles = match env("VICTORY_TEST_TITLES") {
            Some(v) if !v.is_empty() => std::env::split_paths(&v)
                .filter(|p| !p.as_os_str().is_empty())
                .collect(),
            _ => file.test_titles,
        };
        let cache_dir = env_path("VICTORY_CACHE_DIR")
            .or(file.cache_dir)
            .map_or_else(|| default_dir("cache"), Ok)?;
        let data_dir = env_path("VICTORY_DATA_DIR")
            .or(file.data_dir)
            .map_or_else(|| default_dir("data"), Ok)?;
        Ok(Config {
            game_dir: env_path("VICTORY_GAME_DIR").or(file.game_dir),
            cache_dir,
            data_dir,
            test_titles,
            source,
            pad_map: None,
        })
    }

    /// The game directory, or a helpful error if it is not configured.
    pub fn require_game_dir(&self) -> Result<&Path> {
        self.game_dir.as_deref().context(
            "no game directory configured: set VICTORY_GAME_DIR or [paths].game_dir in \
             victory.toml (see victory.example.toml)",
        )
    }

    /// Returns `<cache>/<sub>`, creating it, after checking the asset policy.
    pub fn cache_subdir(&self, sub: &str) -> Result<PathBuf> {
        subdir(&self.cache_dir, sub)
    }

    /// Returns `<data>/<sub>`, creating it, after checking the asset policy.
    pub fn data_subdir(&self, sub: &str) -> Result<PathBuf> {
        subdir(&self.data_dir, sub)
    }
}

/// `VICTORY_PAD_MAP`, else `[input].pad_map`; blank values count as unset.
fn pad_map(file: FileInput, env: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<String> {
    env("VICTORY_PAD_MAP")
        .map(|v| v.to_string_lossy().into_owned())
        .or(file.pad_map)
        .filter(|v| !v.trim().is_empty())
}

fn subdir(root: &Path, sub: &str) -> Result<PathBuf> {
    let dir = root.join(sub);
    ensure_outside_repo(&dir)?;
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir)
}

/// `%LOCALAPPDATA%/victory/<name>` (elsewhere the XDG or home cache).
fn default_dir(name: &str) -> Result<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .context("cannot determine a user directory; set VICTORY_CACHE_DIR and VICTORY_DATA_DIR")?;
    Ok(base.join("victory").join(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn environment_overrides_file() {
        let file = FilePaths {
            game_dir: Some("from-file".into()),
            cache_dir: Some("cache-from-file".into()),
            ..Default::default()
        };
        let env = |k: &str| (k == "VICTORY_GAME_DIR").then(|| OsString::from("from-env"));
        let cfg = Config::resolve(file, None, env).unwrap();
        assert_eq!(cfg.game_dir.unwrap(), PathBuf::from("from-env"));
        assert_eq!(cfg.cache_dir, PathBuf::from("cache-from-file"));
    }

    #[test]
    fn empty_env_values_are_ignored() {
        let file = FilePaths {
            game_dir: Some("from-file".into()),
            ..Default::default()
        };
        let env = |k: &str| (k == "VICTORY_GAME_DIR").then(OsString::new);
        let cfg = Config::resolve(file, None, env).unwrap();
        assert_eq!(cfg.game_dir.unwrap(), PathBuf::from("from-file"));
    }

    #[test]
    fn empty_test_titles_env_is_ignored() {
        let file = FilePaths {
            test_titles: vec!["from-file".into()],
            ..Default::default()
        };
        let env = |k: &str| (k == "VICTORY_TEST_TITLES").then(OsString::new);
        let cfg = Config::resolve(file, None, env).unwrap();
        assert_eq!(cfg.test_titles, vec![PathBuf::from("from-file")]);
    }

    #[test]
    fn test_titles_env_splits_paths() {
        let value = std::env::join_paths(["first", "second"]).unwrap();
        let file = FilePaths::default();
        let env = |k: &str| (k == "VICTORY_TEST_TITLES").then(|| value.clone());
        let cfg = Config::resolve(file, None, env).unwrap();
        assert_eq!(
            cfg.test_titles,
            vec![PathBuf::from("first"), PathBuf::from("second")]
        );
    }

    #[test]
    fn pad_map_comes_from_the_file_unless_the_environment_sets_one() {
        let file = || FileInput {
            pad_map: Some("Space=A".into()),
        };
        let none = |_: &str| None;
        assert_eq!(pad_map(file(), none).as_deref(), Some("Space=A"));
        let env = |k: &str| (k == "VICTORY_PAD_MAP").then(|| OsString::from("R=LB"));
        assert_eq!(pad_map(file(), env).as_deref(), Some("R=LB"));
        // A blank setting anywhere means none.
        let blank = |k: &str| (k == "VICTORY_PAD_MAP").then(|| OsString::from(" "));
        assert_eq!(pad_map(FileInput::default(), blank), None);
        assert_eq!(pad_map(FileInput::default(), none), None);
    }

    #[test]
    fn input_section_parses() {
        let parsed: FileConfig = toml::from_str("[input]\npad_map = 'Space=A,R=LB'").unwrap();
        assert_eq!(parsed.input.pad_map.as_deref(), Some("Space=A,R=LB"));
        // A misspelt key is an error, not silently ignored.
        assert!(toml::from_str::<FileConfig>("[input]\npadmap = 'x'").is_err());
    }

    #[test]
    fn example_config_parses() {
        let text = include_str!("../../../victory.example.toml");
        let parsed: FileConfig = toml::from_str(text).unwrap();
        assert!(parsed.paths.game_dir.is_some());
    }

    #[test]
    fn cache_inside_repo_is_refused() {
        let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("cache");
        let cfg = Config {
            cache_dir: inside,
            ..Default::default()
        };
        assert!(cfg.cache_subdir("x").is_err());
    }

    #[test]
    fn data_inside_repo_is_refused() {
        let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let cfg = Config {
            data_dir: inside,
            ..Default::default()
        };
        assert!(cfg.data_subdir("x").is_err());
    }
}
