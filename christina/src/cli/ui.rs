use console::{Emoji, Style, Term, style};
use dialoguer::{Confirm, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};

// =================================================================================
//  CONSTANTS & THEME
// =================================================================================

pub const CHECK: Emoji<'_, '_> = Emoji("✓ ", "+ ");
pub const CROSS: Emoji<'_, '_> = Emoji("✗ ", "x ");
pub const WARN: Emoji<'_, '_> = Emoji("⚠ ", "! ");
pub const INFO: Emoji<'_, '_> = Emoji("ℹ ", "i ");
pub const ARROW: Emoji<'_, '_> = Emoji("➜ ", "> ");

// =================================================================================
//  COLOR / STYLING UTILITIES
// =================================================================================

pub fn header_style() -> Style {
    Style::new().bold()
}

pub fn success_style() -> Style {
    Style::new().green()
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

pub fn print_header() {
    let term = Term::stdout();
    let version = env!("CARGO_PKG_VERSION");
    let _ = term.write_line(&format!(
        "{} {} {}",
        Emoji("✨", "*"),
        header_style().apply_to("christina"),
        muted_style().apply_to(format!("v{}", version))
    ));
    let _ = term.write_line("");
}

// =================================================================================
//  PROGRESS / SPINNER UTILITIES
// =================================================================================

pub fn create_spinner(msg: &str) -> ProgressBar {
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
    let _ = term.write_line(&format!("{} {}", success_style().apply_to(CHECK), msg));
}

pub fn print_error(msg: &str) {
    let term = Term::stderr();
    let _ = term.write_line(&format!("{} {}", error_style().apply_to(CROSS), msg));
}

pub fn print_warning(msg: &str) {
    let term = Term::stdout();
    let _ = term.write_line(&format!("{} {}", warning_style().apply_to(WARN), msg));
}

pub fn print_info(msg: &str) {
    let term = Term::stdout();
    let _ = term.write_line(&format!("{} {}", accent_style().apply_to(INFO), msg));
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
        success_prefix: style(format!("{}", CHECK)).green(),
        success_suffix: style(String::from("·")).dim(),
        error_prefix: style(format!("{}", CROSS)).red(),
        error_style: error_style(),
        hint_style: muted_style(),
        values_style: accent_style(),
        active_item_style: accent_style(),
        inactive_item_style: Style::new(),
        active_item_prefix: style(format!("{}", ARROW)).cyan(),
        inactive_item_prefix: style(String::from("  ")),
        checked_item_prefix: style(format!("{}", CHECK)).green(),
        unchecked_item_prefix: style(String::from("  ")),
        picked_item_prefix: style(format!("{}", ARROW)).cyan(),
        unpicked_item_prefix: style(String::from("  ")),
    }
}

pub fn confirm(msg: &str) -> Result<bool, dialoguer::Error> {
    Confirm::with_theme(&get_theme())
        .with_prompt(msg)
        .interact()
}

// =================================================================================
//  CONTENT DISPLAY
// =================================================================================

pub fn print_commit_message(msg: &str) {
    let term = Term::stdout();
    let width = 60;

    let border_style = muted_style();

    let _ = term.write_line(&format!(
        "{}",
        border_style.apply_to(format!("┌{}┐", "─".repeat(width)))
    ));

    for line in msg.lines() {
        let chars_count = line.chars().count();
        if chars_count > width {
            let _ = term.write_line(&format!("│{}│", &line[..width]));
        } else {
            let padding = " ".repeat(width - chars_count);
            let _ = term.write_line(&format!("│{}{}│", line, padding));
        }
    }

    let _ = term.write_line(&format!(
        "{}",
        border_style.apply_to(format!("└{}┘", "─".repeat(width)))
    ));
}

pub fn print_file_list(files: &[String]) {
    let term = Term::stdout();
    if files.is_empty() {
        let _ = term.write_line(&format!("{}", muted_style().apply_to("No files changed.")));
        return;
    }

    let _ = term.write_line(&format!("{}", muted_style().apply_to("Changed files:")));
    for file in files {
        let icon = if file.ends_with(".rs") {
            "🦀"
        } else if file.ends_with(".toml") {
            "📦"
        } else if file.ends_with(".md") {
            "📝"
        } else if file.ends_with(".json") {
            "🔧"
        } else if file.ends_with(".lock") {
            "🔒"
        } else if file.starts_with('.') {
            "⚙️ "
        } else {
            "📄"
        };

        let _ = term.write_line(&format!("  {} {}", icon, file));
    }
    let _ = term.write_line("");
}

pub fn print_diff_preview(diff: &str, max_lines: usize) {
    let term = Term::stdout();
    let lines: Vec<&str> = diff.lines().take(max_lines).collect();

    let _ = term.write_line(&format!("{}", muted_style().apply_to("Diff preview:")));

    for line in lines {
        if line.starts_with('+') {
            let _ = term.write_line(&format!("{}", success_style().apply_to(line)));
        } else if line.starts_with('-') {
            let _ = term.write_line(&format!("{}", error_style().apply_to(line)));
        } else if line.starts_with("@@") {
            let _ = term.write_line(&format!("{}", accent_style().apply_to(line)));
        } else {
            let _ = term.write_line(&format!("{}", muted_style().apply_to(line)));
        }
    }

    if diff.lines().count() > max_lines {
        let _ = term.write_line(&format!(
            "{}",
            muted_style().apply_to(format!(
                "... ({} more lines)",
                diff.lines().count() - max_lines
            ))
        ));
    }
    let _ = term.write_line("");
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
