use crate::config::AppConfig;
use crate::data::{Category, Idea, Library, Story};
use crate::data::frontmatter::FrontmatterDoc;
use crate::llm::LlmConfig;
use anyhow::Result;
use ulid::Ulid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Ideas,
    Categories,
    Stories,
    Settings,
}

impl Default for Tab {
    fn default() -> Self { Tab::Ideas }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Llm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    None,
    Idea(Ulid),
    Category(Ulid),
    Story(Ulid),
    Settings(SettingsSection),
}

impl Default for Selection {
    fn default() -> Self { Selection::None }
}

pub struct AppState {
    pub config: AppConfig,
    pub library: Option<Library>,

    pub ideas: Vec<Idea>,
    pub categories: Vec<Category>,
    pub stories: Vec<Story>,

    pub tab: Tab,
    pub selection: Selection,
    pub status: Option<String>,
}

impl AppState {
    pub fn load_from_disk() -> Self {
        let config = AppConfig::load().unwrap_or_default();
        let library = config.data_dir.clone().map(Library::new);
        let mut state = Self {
            config,
            library,
            ideas: Vec::new(),
            categories: Vec::new(),
            stories: Vec::new(),
            tab: Tab::default(),
            selection: Selection::None,
            status: None,
        };
        if let Err(e) = state.reload_all() {
            state.status = Some(format!("初期読み込みに失敗: {e}"));
        }
        state
    }

    pub fn llm(&self) -> &LlmConfig {
        &self.config.llm
    }

    pub fn save_config(&self) -> Result<()> {
        self.config.save()
    }

    pub fn reload_all(&mut self) -> Result<()> {
        let Some(lib) = &self.library else { return Ok(()); };
        self.ideas = lib.load_ideas()?;
        self.categories = lib.load_categories()?;
        self.stories = lib.load_stories()?;
        Ok(())
    }

    pub fn set_data_dir(&mut self, dir: std::path::PathBuf) -> Result<()> {
        self.config.set_data_dir(dir.clone())?;
        self.library = Some(Library::new(dir));
        self.reload_all()
    }

    pub fn idea(&self, id: Ulid) -> Option<&Idea> {
        self.ideas.iter().find(|i| i.meta.id == id)
    }
    pub fn idea_mut(&mut self, id: Ulid) -> Option<&mut Idea> {
        self.ideas.iter_mut().find(|i| i.meta.id == id)
    }
    pub fn category(&self, id: Ulid) -> Option<&Category> {
        self.categories.iter().find(|c| c.meta.id == id)
    }
    pub fn category_mut(&mut self, id: Ulid) -> Option<&mut Category> {
        self.categories.iter_mut().find(|c| c.meta.id == id)
    }
    pub fn story(&self, id: Ulid) -> Option<&Story> {
        self.stories.iter().find(|s| s.meta.id == id)
    }
    pub fn story_mut(&mut self, id: Ulid) -> Option<&mut Story> {
        self.stories.iter_mut().find(|s| s.meta.id == id)
    }

    pub fn create_idea(&mut self) -> Result<Ulid> {
        let lib = self.library.as_ref().expect("library required");
        let idea = Idea::new(&lib.ideas_dir(), "新しいアイデア".into());
        idea.save()?;
        let id = idea.meta.id;
        self.ideas.push(idea);
        Ok(id)
    }
    pub fn create_category(&mut self) -> Result<Ulid> {
        let lib = self.library.as_ref().expect("library required");
        let cat = Category::new(&lib.categories_dir(), "新しいカテゴリ".into());
        cat.save()?;
        let id = cat.meta.id;
        self.categories.push(cat);
        Ok(id)
    }
    pub fn create_story(&mut self) -> Result<Ulid> {
        let lib = self.library.as_ref().expect("library required");
        let story = Story::new(&lib.stories_dir(), "新しいストーリー".into());
        story.save()?;
        let id = story.meta.id;
        self.stories.push(story);
        Ok(id)
    }

    pub fn delete_idea(&mut self, id: Ulid) -> Result<()> {
        if let Some(pos) = self.ideas.iter().position(|i| i.meta.id == id) {
            let idea = self.ideas.remove(pos);
            idea.delete()?;
        }
        Ok(())
    }
    pub fn delete_category(&mut self, id: Ulid) -> Result<()> {
        if let Some(pos) = self.categories.iter().position(|c| c.meta.id == id) {
            let cat = self.categories.remove(pos);
            cat.delete()?;
        }
        Ok(())
    }
    pub fn delete_story(&mut self, id: Ulid) -> Result<()> {
        if let Some(pos) = self.stories.iter().position(|s| s.meta.id == id) {
            let story = self.stories.remove(pos);
            story.delete()?;
        }
        Ok(())
    }
}
