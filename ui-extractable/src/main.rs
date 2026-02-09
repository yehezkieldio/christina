mod primitive;

use std::thread;
use std::time::Duration;

use primitive as ui;

fn main() {
    ui::print_rule(ui::RuleWeight::Light, ui::RuleLength::Full);
    ui::print_heading("UI primitives");
    ui::print_line(ui::LineKind::Info, "Starting demo output.");
    ui::print_line(ui::LineKind::Warning, "Warnings show with a bold exclamation.");
    ui::print_line(ui::LineKind::Error, "Errors go to stderr with emphasis.");
    ui::print_line(ui::LineKind::Success, "Success messages use a muted check.");
    ui::print_line(ui::LineKind::Trace, "Trace lines are muted and go to stderr.");
    ui::print_line(ui::LineKind::Hint, "Hints can guide the next action.");
    ui::print_divider_half();

    ui::print_heading("Lists");
    let files = vec!["src/main.rs", "src/primitive/mod.rs", "Cargo.toml", "README.md"];
    ui::print_list(
        &files,
        ui::ListOptions {
            bullet: "·",
            indent: 2,
            max_items: Some(3),
            muted: true,
            trailing_blank: true,
        },
    );

    ui::print_heading("Blocks");
    ui::print_block("Reusable log fragments can be composed across CLIs without domain coupling.");

    ui::print_heading("Steps");
    ui::print_step(1, 5, "Verify git repository...");
    ui::print_step(2, 5, "Detect workspace type...");
    ui::print_line_indented(ui::LineKind::Success, "Single Rust package", 2);
    ui::print_step(3, 5, "Generate configuration...");
    ui::print_line_indented(ui::LineKind::Success, "default", 2);
    ui::print_step(4, 5, "Write config files...");
    ui::print_line_indented(
        ui::LineKind::Success,
        "yinlin.toml written, cliff.toml written",
        2,
    );
    ui::print_step(5, 5, "Finalize outputs...");
    ui::print_line_indented(ui::LineKind::Info, "Artifacts ready", 2);

    ui::print_badge(ui::BadgeKind::Success, "Created yinlin.toml");
    ui::print_line_indented(
        ui::LineKind::Info,
        "Edit the configuration to match your project",
        2,
    );
    ui::print_line_indented(
        ui::LineKind::Info,
        "Run 'yinlin check' to validate your workspace",
        2,
    );
    ui::print_line_indented(
        ui::LineKind::Info,
        "Run 'yinlin plan' to generate a release plan",
        2,
    );
    ui::print_spacing(1);

    ui::print_line(ui::LineKind::Warning, "2 issue(s) found:");
    ui::print_custom_line(
        "i",
        ui::info_style(),
        None,
        "Package 'test-yinlin-cargo' is pre-1.0 (0.1.0)",
        2,
        false,
    );
    ui::print_line_indented(ui::LineKind::Warning, "Remote 'origin' not configured", 2);
    ui::print_line_indented(
        ui::LineKind::Hint,
        "Fix: Configure remote with 'git remote add origin <url>'",
        4,
    );

    ui::print_underlined_heading("Packages", ui::RuleWeight::Light);
    let packages = vec!["test-yinlin-cargo @ 0.1.0"];
    ui::print_list(
        &packages,
        ui::ListOptions {
            bullet: "•",
            indent: 2,
            max_items: None,
            muted: false,
            trailing_blank: true,
        },
    );

    ui::print_underlined_heading("Pending Changes", ui::RuleWeight::Light);
    ui::print_badge(ui::BadgeKind::Success, "No unreleased changes detected");

    ui::print_underlined_heading("Release Plan Summary", ui::RuleWeight::Heavy);
    ui::print_key_value("Plan Version", "v0.2.0");
    ui::print_key_value("Generated", "2026-02-09 03:41:58 UTC");
    ui::print_key_value("Git SHA", "7baf49e");
    ui::print_key_value("Branch", "master");
    ui::print_key_value("Repository", "clean");
    ui::print_spacing(1);

    let headers = ["Package", "Current", "→", "New", "Bump"];
    let rows = vec![vec![
        "test-yinlin-cargo".to_string(),
        "0.1.0".to_string(),
        "→".to_string(),
        "0.2.0".to_string(),
        "Minor".to_string(),
    ]];
    ui::print_table(&headers, &rows);

    ui::print_line(ui::LineKind::Info, "Tags");
    let tags = vec!["v0.2.0"];
    ui::print_list(
        &tags,
        ui::ListOptions {
            bullet: "•",
            indent: 2,
            max_items: None,
            muted: false,
            trailing_blank: true,
        },
    );

    ui::print_line(ui::LineKind::Info, "Changelog: enabled");
    ui::print_badge(ui::BadgeKind::Success, "Plan written to .yinlin/plan.json");
    ui::print_divider_heavy();

    ui::print_line_to(
        ui::LineKind::Warning,
        "This warning is routed to stderr intentionally.",
        true,
    );

    ui::print_section("Compatibility helpers");
    ui::print_divider();
    ui::print_info("Legacy aliases still map to the new primitives.");
    ui::print_success("Success via alias.");
    ui::print_warning("Warning via alias.");
    ui::print_error("Error via alias.");
    ui::print_trace("Trace via alias.");
    ui::print_commit_message("This block uses the legacy commit-message helper.");
    let legacy_files = vec![
        "src/main.rs".to_string(),
        "src/primitive/mod.rs".to_string(),
        "Cargo.toml".to_string(),
    ];
    ui::print_file_list(&legacy_files, 2);
    ui::print_badge(ui::BadgeKind::Info, "Info badge");
    ui::print_badge(ui::BadgeKind::Warning, "Warning badge");
    ui::print_badge(ui::BadgeKind::Error, "Error badge");
    ui::print_custom_line("★", ui::accent_style(), None, "Accent style sample", 0, false);

    let _accent = ui::accent_style();
    let _theme = ui::theme();
    let spinner = ui::create_spinner("Rendering spinner demo");
    thread::sleep(Duration::from_millis(450));
    spinner.finish_and_clear();
    ui::print_info("Spinner done.");
}
