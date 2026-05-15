use crate::config::AppConfig;
use crate::data::frontmatter::FrontmatterDoc;
use crate::data::{Category, Idea, Library, Story};
use crate::llm::LlmConfig;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use ulid::Ulid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Ideas,
    Categories,
    Stories,
    Settings,
}

impl Default for Tab {
    fn default() -> Self {
        Tab::Ideas
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Llm,
    Tts,
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
    fn default() -> Self {
        Selection::None
    }
}

#[derive(Debug, Clone)]
pub struct Proposal {
    pub id: u64,
    pub raw: String,
    pub pending: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FillProposal {
    pub id: u64,
    pub raw: String,
    pub pending: bool,
    pub error: Option<String>,
    pub hints: Vec<String>,
}

#[derive(Debug)]
pub enum StoryEvent {
    ProposalDelta { pid: u64, chunk: String },
    ProposalsDone { placeholder_pid: u64, raw: String },
    ProposalsError { placeholder_pid: u64, error: String },
    ProposalsCancelled { placeholder_pid: u64 },

    AutoDelta { chunk: String },
    AutoDone { raw: String, cancelled: bool },
    AutoError { error: String },

    CompactDelta { chunk: String },
    CompactDone { new_body: String },
    CompactError { error: String },

    FillDelta { pid: u64, chunk: String },
    FillDone { pid: u64, raw: String },
    FillError { pid: u64, error: String },
    FillCancelled { pid: u64 },

    TitleDelta { chunk: String },
    TitleDone { raw: String },
    TitleError { error: String },
    TitleCancelled,

    TtsDone,
    TtsError { error: String },
}

pub struct StoryEditorState {
    pub count: u32,
    pub proposals: Vec<Proposal>,
    pub fill_proposals: Vec<FillProposal>,
    pub next_proposal_id: u64,

    pub generating: bool,
    pub auto_running: bool,
    pub auto_progress: (u32, u32),
    pub auto_live: String,

    pub compacting: bool,
    pub compact_live: String,
    pub compact_error: Option<String>,

    pub fill_generating: bool,
    pub fill_error: Option<String>,

    pub title_generating: bool,
    pub title_live: String,
    pub title_candidates: Vec<String>,
    pub title_error: Option<String>,

    /// TTS playback state. `tts_cancel` is separate from `cancel_flag` because
    /// audio playback shouldn't be interrupted by generation cancel buttons,
    /// and vice versa.
    pub tts_playing: bool,
    pub tts_error: Option<String>,
    pub tts_cancel: Arc<AtomicBool>,

    /// Latest text selection inside the body TextEdit, as *character* indices.
    /// `None` when nothing was ever selected; `Some((a, b))` with a == b means
    /// the caret is at that position with no range selected.
    pub body_selection: Option<(usize, usize)>,

    pub cancel_flag: Arc<AtomicBool>,
    pub tx: mpsc::Sender<StoryEvent>,
    pub rx: mpsc::Receiver<StoryEvent>,
}

impl StoryEditorState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            count: 3,
            proposals: Vec::new(),
            fill_proposals: Vec::new(),
            next_proposal_id: 1,
            generating: false,
            auto_running: false,
            auto_progress: (0, 0),
            auto_live: String::new(),
            compacting: false,
            compact_live: String::new(),
            compact_error: None,
            fill_generating: false,
            fill_error: None,
            title_generating: false,
            title_live: String::new(),
            title_candidates: Vec::new(),
            title_error: None,
            tts_playing: false,
            tts_error: None,
            tts_cancel: Arc::new(AtomicBool::new(false)),
            body_selection: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            tx,
            rx,
        }
    }

    pub fn next_pid(&mut self) -> u64 {
        let p = self.next_proposal_id;
        self.next_proposal_id += 1;
        p
    }

    pub fn busy(&self) -> bool {
        self.generating
            || self.auto_running
            || self.compacting
            || self.fill_generating
            || self.title_generating
    }
}

#[derive(Debug, Clone)]
pub enum ProbeState {
    Idle,
    Running,
    Ok(Vec<String>),
    Err(String),
}

#[derive(Debug)]
pub enum SettingsEvent {
    ProbeOk(Vec<String>),
    ProbeErr(String),
}

pub struct LlmSettingsState {
    pub probe: ProbeState,
    pub tx: mpsc::Sender<SettingsEvent>,
    pub rx: mpsc::Receiver<SettingsEvent>,
}

impl LlmSettingsState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            probe: ProbeState::Idle,
            tx,
            rx,
        }
    }
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

    pub rt: tokio::runtime::Handle,
    pub egui_ctx: egui::Context,
    pub editors: HashMap<Ulid, StoryEditorState>,
    pub settings: LlmSettingsState,
}

impl AppState {
    pub fn new(rt: tokio::runtime::Handle, egui_ctx: egui::Context) -> Self {
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
            rt,
            egui_ctx,
            editors: HashMap::new(),
            settings: LlmSettingsState::new(),
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
        let Some(lib) = &self.library else {
            return Ok(());
        };
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
        self.editors.remove(&id);
        Ok(())
    }

    pub fn save_idea_now(&mut self, id: Ulid) {
        if let Some(idea) = self.idea_mut(id) {
            idea.meta.updated_at = chrono::Utc::now();
            if let Err(e) = idea.save() {
                tracing::error!(?e, "save idea");
            }
        }
    }
    pub fn save_category_now(&mut self, id: Ulid) {
        if let Some(c) = self.category_mut(id) {
            if let Err(e) = c.save() {
                tracing::error!(?e, "save category");
            }
        }
    }
    pub fn save_story_now(&mut self, id: Ulid) {
        if let Some(s) = self.story_mut(id) {
            if let Err(e) = s.save() {
                tracing::error!(?e, "save story");
            }
        }
    }
}
