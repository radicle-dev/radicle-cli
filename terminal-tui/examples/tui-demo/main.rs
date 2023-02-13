use anyhow::Result;

use radicle_terminal_tui as tui;
use tui::Window;

/// App-specific window implementation.
mod app;

/// Event handler implementations for app-specific message types, since
/// these are not known to `terminal-tui` and need to exist for every component
/// used.
mod components;

/// Runs a basic tui-application with a title, some content and shortcuts.
fn main() -> Result<()> {
    let mut window = Window::default();
    window.run(&mut app::Demo::default())?;

    Ok(())
}
