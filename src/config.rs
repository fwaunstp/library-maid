use crate::llm::LlmConfig;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const QUALIFIER: &str = "net";
const ORG: &str = "fireturtle";
const APP: &str = "library-maid";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub data_dir: Option<PathBuf>,
    pub llm: LlmConfig,
}

impl AppConfig {
    pub fn config_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from(QUALIFIER, ORG, APP)
            .context("could not determine config directory for this platform")?;
        let dir = dirs.config_dir().to_path_buf();
        std::fs::create_dir_all(&dir).with_context(|| format!("create config dir {dir:?}"))?;
        Ok(dir.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("read config {path:?}"))?;
        let cfg: AppConfig = toml::from_str(&raw)
            .with_context(|| format!("parse config {path:?}"))?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let raw = toml::to_string_pretty(self).context("serialize config")?;
        std::fs::write(&path, raw).with_context(|| format!("write config {path:?}"))?;
        Ok(())
    }

    pub fn set_data_dir(&mut self, dir: PathBuf) -> Result<()> {
        ensure_data_layout(&dir)?;
        self.data_dir = Some(dir);
        self.save()
    }
}

pub fn ensure_data_layout(root: &Path) -> Result<()> {
    for sub in ["ideas", "categories", "stories", "prompts"] {
        std::fs::create_dir_all(root.join(sub))
            .with_context(|| format!("create {sub} subdir under {root:?}"))?;
    }
    Ok(())
}

pub fn pick_data_dir() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("library-maid のデータ保存先を選択")
        .pick_folder()
}
