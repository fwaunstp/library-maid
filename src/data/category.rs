use super::frontmatter::FrontmatterDoc;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ulid::Ulid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryMeta {
    pub id: Ulid,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Category {
    pub path: PathBuf,
    pub meta: CategoryMeta,
    pub body: String,
}

impl Category {
    pub fn new(dir: &Path, name: String) -> Self {
        let id = Ulid::new();
        let path = dir.join(format!("{id}.md"));
        Self {
            path,
            meta: CategoryMeta { id, name },
            body: String::new(),
        }
    }

    pub fn delete(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

impl FrontmatterDoc for Category {
    type Meta = CategoryMeta;

    fn from_parts(path: PathBuf, meta: CategoryMeta, body: String) -> Self {
        Self { path, meta, body }
    }
    fn meta(&self) -> &CategoryMeta { &self.meta }
    fn body(&self) -> &str { &self.body }
    fn path(&self) -> &Path { &self.path }
}
