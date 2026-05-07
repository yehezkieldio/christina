//! Primitive terminal UI components for CLI applications.

use console::{Color, Style, Term, measure_text_width, style};
use dialoguer::theme::ColorfulTheme;
use indicatif::{ProgressBar, ProgressStyle};

const MAX_RULE_WIDTH: usize = 80;

#[derive(Clone, Copy, Debug)]
pub enum LineKind {
    Info,
    Warning,
    Error,
    Success,
    Trace,
    Hint,
}

#[derive(Clone, Copy, Debug)]
pub enum BadgeKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug)]
pub enum RuleWeight {
    Light,
    Heavy,
}

#[derive(Clone, Copy, Debug)]
pub enum RuleLength {
    Full,
    Half,
}

#[derive(Clone, Debug)]
pub struct ListOptions<'a> {
    pub bullet: &'a str,
    pub indent: usize,
    pub max_items: Option<usize>,
    pub muted: bool,
    pub trailing_blank: bool,
}

impl<'a> Default for ListOptions<'a> {
    fn default() -> Self {
        Self {
            bullet: "•",
            indent: 0,
            max_items: None,
            muted: false,
            trailing_blank: false,
        }
    }
}

pub fn header_style() -> Style {
    Style::new().bold()
}

pub fn error_style() -> Style {
    Style::new().fg(Color::Red).bold()
}

pub fn warning_style() -> Style {
    Style::new().fg(Color::Yellow).bold()
}

pub fn accent_style() -> Style {
    Style::new().fg(Color::Cyan).bold()
}

pub fn muted_style() -> Style {
    Style::new().dim()
}

pub fn info_style() -> Style {
    Style::new().fg(Color::Blue)
}

pub fn success_style() -> Style {
    Style::new().fg(Color::Green).dim()
}

pub fn hint_style() -> Style {
    Style::new().fg(Color::Magenta).dim()
}

pub fn trace_style() -> Style {
    Style::new().dim()
}

struct LineStyle {
    symbol: &'static str,
    symbol_style: Style,
    text_style: Option<Style>,
    default_stderr: bool,
}

pub fn create_spinner(msg: &str) -> ProgressBar {
    let term = Term::stdout();
    if !term.is_term() {
        // Non-TTY output (e.g., pipes) should avoid spinner control codes.
        let pb = ProgressBar::hidden();
        pb.set_message(msg.to_string());
        return pb;
    }

    let pb = ProgressBar::new_spinner();
    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈")
        .template("{spinner} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner().tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"));
    pb.set_style(spinner_style);
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(120));
    pb
}

fn line_style(kind: LineKind) -> LineStyle {
    match kind {
        LineKind::Info => LineStyle {
            symbol: "•",
            symbol_style: accent_style(),
            text_style: None,
            default_stderr: false,
        },
        LineKind::Warning => LineStyle {
            symbol: "!",
            symbol_style: warning_style(),
            text_style: None,
            default_stderr: false,
        },
        LineKind::Error => LineStyle {
            symbol: "×",
            symbol_style: error_style(),
            text_style: Some(error_style()),
            default_stderr: true,
        },
        LineKind::Success => LineStyle {
            symbol: "✓",
            symbol_style: success_style(),
            text_style: None,
            default_stderr: false,
        },
        LineKind::Trace => LineStyle {
            symbol: "·",
            symbol_style: trace_style(),
            text_style: Some(trace_style()),
            default_stderr: true,
        },
        LineKind::Hint => LineStyle {
            symbol: "→",
            symbol_style: hint_style(),
            text_style: Some(muted_style()),
            default_stderr: false,
        },
    }
}

pub fn print_line(kind: LineKind, msg: &str) {
    print_line_with(kind, msg, 0, None);
}

pub fn print_line_indented(kind: LineKind, msg: &str, indent: usize) {
    print_line_with(kind, msg, indent, None);
}

pub fn print_line_to(kind: LineKind, msg: &str, to_stderr: bool) {
    print_line_with(kind, msg, 0, Some(to_stderr));
}

pub fn print_custom_line(
    symbol: &str,
    symbol_style: Style,
    text_style: Option<Style>,
    msg: &str,
    indent: usize,
    to_stderr: bool,
) {
    let term = if to_stderr {
        Term::stderr()
    } else {
        Term::stdout()
    };
    let indent = " ".repeat(indent);
    let body = match text_style.as_ref() {
        Some(style) => format!("{}", style.apply_to(msg)),
        None => msg.to_string(),
    };
    let _ = term.write_line(&format!(
        "{}{} {}",
        indent,
        symbol_style.apply_to(symbol),
        body
    ));
}

fn print_line_with(kind: LineKind, msg: &str, indent: usize, to_stderr: Option<bool>) {
    let style = line_style(kind);
    let to_stderr = to_stderr.unwrap_or(style.default_stderr);
    let term = if to_stderr {
        Term::stderr()
    } else {
        Term::stdout()
    };
    let indent = " ".repeat(indent);
    let body = match style.text_style.as_ref() {
        Some(style) => format!("{}", style.apply_to(msg)),
        None => msg.to_string(),
    };
    let _ = term.write_line(&format!(
        "{}{} {}",
        indent,
        style.symbol_style.apply_to(style.symbol),
        body
    ));
}

pub fn print_badge(kind: BadgeKind, msg: &str) {
    let (symbol, style) = match kind {
        BadgeKind::Info => ("i", info_style()),
        BadgeKind::Success => ("✓", success_style()),
        BadgeKind::Warning => ("!", warning_style()),
        BadgeKind::Error => ("×", error_style()),
    };
    let term = Term::stdout();
    let _ = term.write_line(&format!(
        "{}{}{} {}",
        muted_style().apply_to("["),
        style.apply_to(symbol),
        muted_style().apply_to("]"),
        msg
    ));
}

pub fn print_step(current: usize, total: usize, msg: &str) {
    let term = Term::stdout();
    let counter = format!("{}/{}", current, total);
    let _ = term.write_line(&format!(
        "{}{}{} {}",
        muted_style().apply_to("["),
        accent_style().apply_to(counter),
        muted_style().apply_to("]"),
        msg
    ));
}

pub fn print_heading(title: &str) {
    let term = Term::stdout();
    let _ = term.write_line("");
    let _ = term.write_line(&format!(
        "{} {}",
        muted_style().apply_to("—"),
        header_style().apply_to(title)
    ));
    let _ = term.write_line("");
}

pub fn print_underlined_heading(title: &str, weight: RuleWeight) {
    let term = Term::stdout();
    let _ = term.write_line("");
    let _ = term.write_line(&format!("{}", header_style().apply_to(title)));
    let underline = rule_glyph(weight)
        .to_string()
        .repeat(text_width(title).max(1));
    let _ = term.write_line(&format!("{}", muted_style().apply_to(underline)));
    let _ = term.write_line("");
}

pub fn print_rule(weight: RuleWeight, length: RuleLength) {
    let term = Term::stdout();
    let width = term.size().1 as usize;
    let max_width = width.clamp(1, MAX_RULE_WIDTH);
    let display_width = match length {
        RuleLength::Full => max_width,
        RuleLength::Half => (max_width / 2).max(1),
    };
    let _ = term.write_line(&format!(
        "{}",
        muted_style().apply_to(rule_glyph(weight).to_string().repeat(display_width))
    ));
}

pub fn print_block(text: &str) {
    print_block_with(text, "│", 0, None);
}

pub fn print_block_with(text: &str, prefix: &str, indent: usize, width: Option<usize>) {
    let term = Term::stdout();
    let term_width = term.size().1 as usize;
    let prefix_width = text_width(prefix) + 1;
    let available = width.unwrap_or(term_width).min(96);
    let usable = available.saturating_sub(prefix_width + indent).max(10);
    let indent = " ".repeat(indent);

    for line in wrap_text(text, usable) {
        if line.is_empty() {
            let _ = term.write_line(&format!("{}{}", indent, muted_style().apply_to(prefix)));
        } else {
            let _ = term.write_line(&format!(
                "{}{} {}",
                indent,
                muted_style().apply_to(prefix),
                line
            ));
        }
    }
    let _ = term.write_line("");
}

pub fn print_list<T: AsRef<str>>(items: &[T], options: ListOptions<'_>) {
    let term = Term::stdout();
    if items.is_empty() {
        let msg = "no items";
        let _ = if options.muted {
            term.write_line(&format!("{}", muted_style().apply_to(msg)))
        } else {
            term.write_line(msg)
        };
        return;
    }

    let visible = options
        .max_items
        .map_or(items.len(), |limit| items.len().min(limit));
    let indent = " ".repeat(options.indent);
    for item in items.iter().take(visible) {
        let line = format!("{}{} {}", indent, options.bullet, item.as_ref());
        let _ = if options.muted {
            term.write_line(&format!("{}", muted_style().apply_to(line)))
        } else {
            term.write_line(&line)
        };
    }

    if items.len() > visible {
        let remaining = items.len() - visible;
        let line = format!("… {} more", remaining);
        let _ = if options.muted {
            term.write_line(&format!("{}", muted_style().apply_to(line)))
        } else {
            term.write_line(&line)
        };
    }

    if options.trailing_blank {
        let _ = term.write_line("");
    }
}

pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if headers.is_empty() && rows.is_empty() {
        return;
    }

    let term = Term::stdout();
    let columns = rows
        .iter()
        .map(|row| row.len())
        .max()
        .unwrap_or(0)
        .max(headers.len());
    let mut widths = vec![0usize; columns];

    for (idx, header) in headers.iter().enumerate() {
        widths[idx] = widths[idx].max(text_width(header));
    }

    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(text_width(cell));
        }
    }

    let header_line = format_row(headers.to_vec(), &widths);
    let divider_line = widths
        .iter()
        .map(|width| "─".repeat(*width))
        .collect::<Vec<String>>()
        .join("  ");

    let _ = term.write_line(&format!("{}", header_style().apply_to(header_line)));
    let _ = term.write_line(&format!("{}", muted_style().apply_to(divider_line)));

    for row in rows {
        let row_line = format_row(row.iter().map(|s| s.as_str()).collect(), &widths);
        let _ = term.write_line(&row_line);
    }

    let _ = term.write_line("");
}

pub fn print_key_value(label: &str, value: &str) {
    let term = Term::stdout();
    let _ = term.write_line(&format!("{}: {}", muted_style().apply_to(label), value));
}

pub fn print_spacing(lines: usize) {
    let term = Term::stdout();
    for _ in 0..lines {
        let _ = term.write_line("");
    }
}

pub fn theme() -> ColorfulTheme {
    ColorfulTheme {
        defaults_style: muted_style(),
        prompt_style: Style::new(),
        prompt_prefix: style(String::from("›")).dim(),
        prompt_suffix: style(String::from("")),
        success_prefix: style(String::from("ok")).dim(),
        success_suffix: style(String::from("")),
        error_prefix: style(String::from("error")),
        error_style: error_style(),
        hint_style: muted_style(),
        values_style: accent_style(),
        active_item_style: accent_style(),
        inactive_item_style: Style::new(),
        active_item_prefix: style(String::from("› ")),
        inactive_item_prefix: style(String::from("  ")),
        checked_item_prefix: style(String::from("✓ ")),
        unchecked_item_prefix: style(String::from("  ")),
        picked_item_prefix: style(String::from("› ")),
        unpicked_item_prefix: style(String::from("  ")),
    }
}

fn format_row(cells: Vec<&str>, widths: &[usize]) -> String {
    let mut pieces = Vec::with_capacity(widths.len());
    for (idx, width) in widths.iter().enumerate() {
        let text = cells.get(idx).copied().unwrap_or("");
        pieces.push(pad_to_width(text, *width));
    }
    pieces.join("  ")
}

fn pad_to_width(text: &str, width: usize) -> String {
    let text_width = text_width(text);
    if text_width >= width {
        return text.to_string();
    }
    let mut out = String::with_capacity(width + 1);
    out.push_str(text);
    out.push_str(&" ".repeat(width - text_width));
    out
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        let mut current_width = 0usize;
        for word in raw_line.split_whitespace() {
            let word_width = text_width(word);
            if current.is_empty() {
                current.push_str(word);
                current_width = word_width;
                continue;
            }

            if current_width + 1 + word_width > width {
                lines.push(current);
                current = String::new();
                current.push_str(word);
                current_width = word_width;
            } else {
                current.push(' ');
                current.push_str(word);
                current_width += 1 + word_width;
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    lines
}

fn rule_glyph(weight: RuleWeight) -> char {
    match weight {
        RuleWeight::Light => '─',
        RuleWeight::Heavy => '═',
    }
}

fn text_width(text: &str) -> usize {
    measure_text_width(text)
}

pub fn print_success(msg: &str) {
    print_line(LineKind::Success, msg);
}

pub fn print_error(msg: &str) {
    print_line(LineKind::Error, msg);
}

pub fn print_warning(msg: &str) {
    print_line(LineKind::Warning, msg);
}

pub fn print_info(msg: &str) {
    print_line(LineKind::Info, msg);
}

pub fn print_trace(msg: &str) {
    print_line(LineKind::Trace, msg);
}

pub fn print_section(title: &str) {
    print_heading(title);
}

pub fn print_divider() {
    print_rule(RuleWeight::Light, RuleLength::Full);
}

pub fn print_divider_half() {
    print_rule(RuleWeight::Light, RuleLength::Half);
}

pub fn print_divider_heavy() {
    print_rule(RuleWeight::Heavy, RuleLength::Full);
}

pub fn print_commit_message(msg: &str) {
    print_block(msg);
}

pub fn print_file_list(files: &[String], max_items: usize) {
    print_list(
        files,
        ListOptions {
            bullet: "·",
            indent: 2,
            max_items: Some(max_items),
            muted: true,
            trailing_blank: true,
        },
    );
}
