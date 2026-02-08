//! Terminal UI helpers for consistent formatting and prompts.
//!
//! WHY lightweight: keep UI rendering simple to avoid coupling to heavy TUI
//! frameworks while still offering a polished CLI experience.

pub mod events;

use std::io::IsTerminal;

use console::{Style, Term, style};
use dialoguer::{Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use rustyline::{At, Cmd, Config, EditMode, Editor, KeyCode, KeyEvent, Modifiers, Movement, Word};

// =================================================================================
//  CONSTANTS & THEME
// =================================================================================

// =================================================================================
//  COLOR / STYLING UTILITIES
// =================================================================================

pub fn header_style() -> Style {
    Style::new().bold()
}

pub fn error_style() -> Style {
    Style::new().red()
}

pub fn warning_style() -> Style {
    Style::new().yellow()
}

pub fn accent_style() -> Style {
    Style::new().cyan()
}

pub fn muted_style() -> Style {
    Style::new().dim()
}

// =================================================================================
//  HEADER COMPONENT
// =================================================================================

// pub fn print_header() {
//     let term = Term::stdout();
//     let version = env!("CARGO_PKG_VERSION");
//     let _ = term.write_line(&format!(
//         "{} {}",
//         header_style().apply_to("christina"),
//         muted_style().apply_to(format!("v{}", version))
//     ));
//     let _ = term.write_line("");
// }

// =================================================================================
//  PROGRESS / SPINNER UTILITIES
// =================================================================================

pub fn create_spinner(msg: &str) -> ProgressBar {
    let term = Term::stdout();
    if !term.is_term() {
        // Non-TTY output (e.g., pipes) should avoid spinner control codes.
        let pb = ProgressBar::hidden();
        pb.set_message(msg.to_string());
        return pb;
    }

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

// =================================================================================
//  STATUS MESSAGES
// =================================================================================

pub fn print_success(msg: &str) {
    let term = Term::stdout();
    let _ = term.write_line(&format!("{} {}", muted_style().apply_to("ok"), msg));
}

pub fn print_error(msg: &str) {
    let term = Term::stderr();
    let _ = term.write_line(&format!("{} {}", error_style().apply_to("error"), msg));
}

pub fn print_warning(msg: &str) {
    let term = Term::stdout();
    let _ = term.write_line(&format!("{} {}", warning_style().apply_to("warn"), msg));
}

pub fn print_info(msg: &str) {
    let term = Term::stdout();
    let _ = term.write_line(&format!("{} {}", muted_style().apply_to("info"), msg));
}

pub fn print_trace(msg: &str) {
    let term = Term::stderr();
    let label = muted_style().apply_to("trace");
    let body = muted_style().apply_to(msg);
    let _ = term.write_line(&format!("{} {}", label, body));
}

// =================================================================================
//  INTERACTIVE PROMPTS
// =================================================================================

fn get_theme() -> ColorfulTheme {
    ColorfulTheme {
        defaults_style: muted_style(),
        prompt_style: Style::new(),
        prompt_prefix: style(String::from("?")).dim(),
        prompt_suffix: style(String::from("›")).bold(),
        success_prefix: style(String::from("ok")).green(),
        success_suffix: style(String::from("·")).dim(),
        error_prefix: style(String::from("error")).red(),
        error_style: error_style(),
        hint_style: muted_style(),
        values_style: accent_style(),
        active_item_style: accent_style(),
        inactive_item_style: Style::new(),
        active_item_prefix: style(String::from("> ")).cyan(),
        inactive_item_prefix: style(String::from("  ")),
        checked_item_prefix: style(String::from("ok")).green(),
        unchecked_item_prefix: style(String::from("  ")),
        picked_item_prefix: style(String::from("> ")).cyan(),
        unpicked_item_prefix: style(String::from("  ")),
    }
}

pub fn select_action(actions: &[&str]) -> Result<usize, dialoguer::Error> {
    Select::with_theme(&get_theme())
        .items(actions)
        .default(0)
        .interact()
}

pub fn edit_commit_message_inline(
    initial_content: &str,
) -> Result<Option<String>, std::io::Error> {
    if !Term::stdout().is_term() || !std::io::stdin().is_terminal() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Inline editing requires an interactive terminal",
        ));
    }

    let config = Config::builder()
        .edit_mode(EditMode::Emacs)
        .auto_add_history(false)
        .build();
    let mut editor = Editor::<(), DefaultHistory>::with_config(config).map_err(|err| {
        std::io::Error::other(format!("Failed to initialize line editor: {err}"))
    })?;

    bind_ctrl_word_navigation(&mut editor);

    let mut current = sanitize_inline_message(initial_content);
    loop {
        match editor.readline_with_initial("", (&current, "")) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    print_warning("Commit message cannot be empty.");
                    current = line;
                    continue;
                }
                return Ok(Some(trimmed.to_string()));
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => return Ok(None),
            Err(err) => {
                return Err(std::io::Error::other(format!(
                    "Inline editing failed: {err}"
                )))
            }
        }
    }
}

fn bind_ctrl_word_navigation(editor: &mut Editor<(), DefaultHistory>) {
    let _ = editor.bind_sequence(
        KeyEvent(KeyCode::Left, Modifiers::CTRL),
        Cmd::Move(Movement::BackwardWord(1, Word::Emacs)),
    );
    let _ = editor.bind_sequence(
        KeyEvent(KeyCode::Right, Modifiers::CTRL),
        Cmd::Move(Movement::ForwardWord(1, At::AfterEnd, Word::Emacs)),
    );
}

fn sanitize_inline_message(message: &str) -> String {
    message.lines().collect::<Vec<_>>().join(" ")
}

// =================================================================================
//  CONTENT DISPLAY
// =================================================================================

pub fn print_commit_message(msg: &str) {
    let term = Term::stdout();
    let width = std::cmp::min(term.size().1 as usize, 96);
    let width = width.saturating_sub(2).max(40);

    for line in wrap_text(msg, width) {
        let _ = term.write_line(&line);
    }
    let _ = term.write_line("");
}

pub fn print_file_list(files: &[String], max_items: usize) {
    let term = Term::stdout();
    if files.is_empty() {
        let _ = term.write_line(&format!("{}", muted_style().apply_to("no files changed")));
        return;
    }

    let visible = files.len().min(max_items);
    for file in files.iter().take(visible) {
        let _ = term.write_line(&format!("  {}", file));
    }

    if files.len() > visible {
        let remaining = files.len() - visible;
        let _ = term.write_line(&format!(
            "{}",
            muted_style().apply_to(format!("  ... {} more", remaining))
        ));
    }
    let _ = term.write_line("");
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in raw_line.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
                continue;
            }

            if current.len() + 1 + word.len() > width {
                lines.push(current);
                current = String::new();
                current.push_str(word);
            } else {
                current.push(' ');
                current.push_str(word);
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    lines
}

// =================================================================================
//  SECTION DIVIDERS
// =================================================================================

pub fn print_section(title: &str) {
    let term = Term::stdout();
    let _ = term.write_line("");
    let _ = term.write_line(&format!(
        "{}",
        header_style().apply_to(title.to_uppercase())
    ));
    let _ = term.write_line("");
}

pub fn print_divider() {
    let term = Term::stdout();
    let width = term.size().1 as usize;
    let display_width = std::cmp::min(width, 80);
    let _ = term.write_line(&format!(
        "{}",
        muted_style().apply_to("─".repeat(display_width))
    ));
}
