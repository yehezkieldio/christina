mod primitive;

use std::thread;
use std::time::Duration;

use primitive as ui;

fn main() {
    ui::print_divider();
    ui::print_section("UI primitives");
    ui::print_info("Starting demo output.");
    ui::print_warning("Warnings show with a bold exclamation.");
    ui::print_error("Errors go to stderr with emphasis.");
    ui::print_success("Success messages use a muted check.");
    ui::print_trace("Trace lines are muted and go to stderr.");

    let files = vec![
        "src/main.rs".to_string(),
        "src/primitive/mod.rs".to_string(),
        "Cargo.toml".to_string(),
        "README.md".to_string(),
    ];
    ui::print_section("File list");
    ui::print_file_list(&files, 3);

    ui::print_section("Commit message");
    ui::print_commit_message("feat: extract reusable UI primitives");

    let spinner = ui::create_spinner("Rendering spinner demo");
    thread::sleep(Duration::from_millis(450));
    spinner.finish_and_clear();
    ui::print_info("Spinner done.");
}
