use anyhow::{Context, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};

pub trait FrontmatterDoc: Sized {
    type Meta: Serialize + DeserializeOwned;

    fn from_parts(path: PathBuf, meta: Self::Meta, body: String) -> Self;
    fn meta(&self) -> &Self::Meta;
    fn body(&self) -> &str;
    fn path(&self) -> &Path;

    fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read {path:?}"))?;
        let (meta, body) = split_frontmatter::<Self::Meta>(&raw)
            .with_context(|| format!("parse {path:?}"))?;
        Ok(Self::from_parts(path.to_path_buf(), meta, body))
    }

    fn save(&self) -> Result<()> {
        let raw = render_frontmatter(self.meta(), self.body())
            .with_context(|| format!("serialize {:?}", self.path()))?;
        if let Some(parent) = self.path().parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(self.path(), raw)
            .with_context(|| format!("write {:?}", self.path()))?;
        Ok(())
    }
}

pub fn split_frontmatter<M: DeserializeOwned>(raw: &str) -> Result<(M, String)> {
    let rest = raw
        .strip_prefix("---\n")
        .or_else(|| raw.strip_prefix("---\r\n"))
        .context("missing leading `---` frontmatter delimiter")?;
    let end = rest
        .find("\n---")
        .context("missing trailing `---` frontmatter delimiter")?;
    let yaml = &rest[..end];
    let after = &rest[end + 4..];
    let body = after.trim_start_matches('\r').trim_start_matches('\n').to_string();
    let meta: M = serde_yaml_ng::from_str(yaml).context("parse yaml frontmatter")?;
    Ok((meta, body))
}

pub fn render_frontmatter<M: Serialize>(meta: &M, body: &str) -> Result<String> {
    let yaml = serde_yaml_ng::to_string(meta).context("serialize yaml frontmatter")?;
    Ok(format!("---\n{yaml}---\n{body}"))
}
