use std::io::stdout;

use anyhow::Result;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    crossterm::{
        ExecutableCommand,
        event::{self, Event, KeyEventKind},
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};

use crate::config::Config;

use super::app::ConfigApp;
use super::screen;
use super::update;

pub type SaveCallback = Box<dyn FnMut(&Config) -> Result<()>>;

/// Configuration for running the config TUI
pub struct ConfigTuiOptions {
    /// The configuration data to edit
    pub config: Config,
    /// Whether an API key is set
    pub has_api_key: bool,
    /// Source of the API key (for display)
    pub api_key_source: Option<&'static str>,
    /// Callback to save the configuration
    pub on_save: SaveCallback,
}

/// Result of running the config TUI
pub enum ConfigTuiResult {
    /// User quit normally
    Quit,
    /// User wants to open profile manager
    OpenProfiles,
}

pub fn run_config_tui(mut options: ConfigTuiOptions) -> Result<ConfigTuiResult> {
    // Set up terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let result = run_app(&mut terminal, &mut options);

    // Clean up terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    options: &mut ConfigTuiOptions,
) -> Result<ConfigTuiResult> {
    let mut app = ConfigApp::new(
        options.config.clone(),
        options.has_api_key,
        options.api_key_source,
    );

    loop {
        terminal.draw(|frame: &mut Frame| screen::render(frame, &mut app))?;

        if app.should_quit {
            break;
        }

        // Poll for events with a small timeout
        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            update::handle_key(&mut app, key, &mut options.on_save);
        }
    }

    if app.open_profiles {
        Ok(ConfigTuiResult::OpenProfiles)
    } else {
        Ok(ConfigTuiResult::Quit)
    }
}
