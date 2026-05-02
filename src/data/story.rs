use super::frontmatter::FrontmatterDoc;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ulid::Ulid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryMeta {
    pub id: Ulid,
    pub title: String,
    #[serde(default)]
    pub nsfw: bool,
    /// Ideas the user has explicitly turned on for this story.
    /// `requires`-driven activations are computed at generation time, not stored.
    #[serde(default)]
    pub active_ideas: Vec<Ulid>,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Story {
    pub path: PathBuf,
    pub meta: StoryMeta,
    pub body: String,
}

impl Story {
    pub fn new(dir: &Path, title: String) -> Self {
        let id = Ulid::new();
        let now = Utc::now();
        let path = dir.join(format!("{id}.md"));
        Self {
            path,
            meta: StoryMeta {
                id,
                title,
                nsfw: true,
                active_ideas: Vec::new(),
                created_at: now,
                updated_at: now,
            },
            body: String::new(),
        }
    }

    pub fn append_body(&mut self, chunk: &str) {
        if !self.body.is_empty() && !self.body.ends_with('\n') {
            self.body.push('\n');
        }
        self.body.push_str(chunk);
        self.meta.updated_at = Utc::now();
    }

    pub fn replace_body(&mut self, body: String) {
        self.body = body;
        self.meta.updated_at = Utc::now();
    }

    pub fn delete(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

impl FrontmatterDoc for Story {
    type Meta = StoryMeta;

    fn from_parts(path: PathBuf, meta: StoryMeta, body: String) -> Self {
        Self { path, meta, body }
    }
    fn meta(&self) -> &StoryMeta { &self.meta }
    fn body(&self) -> &str { &self.body }
    fn path(&self) -> &Path { &self.path }
}
