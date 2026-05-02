#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod config;
mod data;
mod llm;
mod prompts;
mod ui;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "library_maid=info,warn".into()),
        )
        .init();

    let window = dioxus::desktop::WindowBuilder::new()
        .with_title("library-maid")
        .with_always_on_top(false);
    let cfg = dioxus::desktop::Config::new().with_window(window);

    dioxus::LaunchBuilder::desktop()
        .with_cfg(cfg)
        .launch(ui::App);
}
