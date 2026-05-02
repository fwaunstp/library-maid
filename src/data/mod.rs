pub mod category;
pub mod frontmatter;
pub mod idea;
pub mod story;

use anyhow::Result;
use std::path::{Path, PathBuf};

pub use category::Category;
pub use idea::Idea;
pub use story::Story;

#[derive(Debug, Clone)]
pub struct Library {
    pub root: PathBuf,
}

impl Library {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn ideas_dir(&self) -> PathBuf { self.root.join("ideas") }
    pub fn categories_dir(&self) -> PathBuf { self.root.join("categories") }
    pub fn stories_dir(&self) -> PathBuf { self.root.join("stories") }
    pub fn prompts_dir(&self) -> PathBuf { self.root.join("prompts") }

    pub fn load_ideas(&self) -> Result<Vec<Idea>> {
        load_all(&self.ideas_dir())
    }
    pub fn load_categories(&self) -> Result<Vec<Category>> {
        load_all(&self.categories_dir())
    }
    pub fn load_stories(&self) -> Result<Vec<Story>> {
        load_all(&self.stories_dir())
    }
}

fn load_all<T: frontmatter::FrontmatterDoc>(dir: &Path) -> Result<Vec<T>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        match T::load(&path) {
            Ok(doc) => out.push(doc),
            Err(e) => tracing::warn!(?path, error = ?e, "skipping malformed file"),
        }
    }
    Ok(out)
}
