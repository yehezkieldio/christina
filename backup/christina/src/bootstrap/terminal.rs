use std::io::{Stdout, stdout};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use ratatui::{
    Terminal as RatatuiTerminal,
    backend::CrosstermBackend,
    crossterm::{
        ExecutableCommand,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};

pub struct TerminalHandle {
    terminal: RatatuiTerminal<CrosstermBackend<Stdout>>,
    cleanup_done: Arc<AtomicBool>,
}

impl TerminalHandle {
    /// Initialize terminal with raw mode and alternate screen.
    pub fn init() -> Result<Self> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;

        let terminal = RatatuiTerminal::new(CrosstermBackend::new(stdout()))?;
        let cleanup_done = Arc::new(AtomicBool::new(false));

        // Set up panic hook to clean up terminal
        let cleanup_done_clone = Arc::clone(&cleanup_done);
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            if !cleanup_done_clone.load(Ordering::SeqCst) {
                let _ = disable_raw_mode();
                let _ = stdout().execute(LeaveAlternateScreen);
            }
            original_hook(panic_info);
        }));

        Ok(Self {
            terminal,
            cleanup_done,
        })
    }

    pub fn terminal_mut(&mut self) -> &mut RatatuiTerminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    pub fn cleanup(self, exit_message: Option<String>) -> Result<()> {
        // Signal that cleanup is done to prevent panic hook from double-cleaning
        self.cleanup_done.store(true, Ordering::SeqCst);

        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;

        if let Some(message) = exit_message {
            println!("{message}");
        }

        Ok(())
    }
}

impl Drop for TerminalHandle {
    fn drop(&mut self) {
        // Ensure cleanup happens even if cleanup() wasn't called explicitly
        if !self.cleanup_done.load(Ordering::SeqCst) {
            self.cleanup_done.store(true, Ordering::SeqCst);
            let _ = disable_raw_mode();
            let _ = stdout().execute(LeaveAlternateScreen);
        }
    }
}
