mod setup;
mod library;
mod editors;
mod settings;
mod state;
mod style;

pub use state::AppState;

use dioxus::prelude::*;

#[component]
pub fn App() -> Element {
    let state = use_context_provider(|| Signal::new(AppState::load_from_disk()));

    rsx! {
        style { "{style::CSS}" }
        div { class: "app-root",
            if state.read().library.is_none() {
                setup::SetupScreen {}
            } else {
                library::LibraryScreen {}
            }
        }
    }
}
