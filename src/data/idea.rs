use super::frontmatter::FrontmatterDoc;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ulid::Ulid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeaMeta {
    pub id: Ulid,
    pub title: String,
    #[serde(default)]
    pub categories: Vec<Ulid>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// All listed ids must be active for this idea to auto-activate.
    #[serde(default)]
    pub requires: Vec<Ulid>,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Idea {
    pub path: PathBuf,
    pub meta: IdeaMeta,
    pub body: String,
}

impl Idea {
    pub fn new(dir: &Path, title: String) -> Self {
        let id = Ulid::new();
        let now = Utc::now();
        let path = dir.join(format!("{id}.md"));
        Self {
            path,
            meta: IdeaMeta {
                id,
                title,
                categories: Vec::new(),
                tags: Vec::new(),
                requires: Vec::new(),
                created_at: now,
                updated_at: now,
            },
            body: String::new(),
        }
    }

    pub fn update_body(&mut self, body: String) {
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

impl FrontmatterDoc for Idea {
    type Meta = IdeaMeta;

    fn from_parts(path: PathBuf, meta: IdeaMeta, body: String) -> Self {
        Self { path, meta, body }
    }
    fn meta(&self) -> &IdeaMeta { &self.meta }
    fn body(&self) -> &str { &self.body }
    fn path(&self) -> &Path { &self.path }
}
