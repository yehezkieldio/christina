use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::diff_executor::ResolvedDiffTool;
use super::theme::*;
use crate::config::DiffTool;
use christina_core::GitFile;

fn compute_hash(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

pub struct DiffRenderer {
    tool: ResolvedDiffTool,
    cached_result: Option<(String, u64, Arc<Vec<Line<'static>>>)>,
}

impl DiffRenderer {
    pub fn new(diff_tool: DiffTool) -> Self {
        Self {
            tool: ResolvedDiffTool::detect_from(diff_tool),
            cached_result: None,
        }
    }

    pub fn is_using_delta(&self) -> bool {
        matches!(self.tool, ResolvedDiffTool::Delta { .. })
    }

    pub fn render_diff(&mut self, file: &GitFile, terminal_width: u16) -> Vec<Line<'static>> {
        if file.is_binary {
            return vec![Line::from(Span::styled(
                "[Binary file - no preview available]",
                Style::default().fg(SUBTEXT0).add_modifier(Modifier::ITALIC),
            ))];
        }

        if file.diff_content.is_empty() {
            return vec![Line::from(Span::styled(
                "[No diff content]",
                Style::default().fg(SUBTEXT0).add_modifier(Modifier::ITALIC),
            ))];
        }

        let file_path = file.path.to_string();
        let content_hash = compute_hash(&file.diff_content);

        if let Some((cached_path, cached_hash, cached_lines)) = &self.cached_result
            && cached_path == &file_path
            && cached_hash == &content_hash
        {
            return cached_lines.as_ref().clone();
        }

        let rendered = self
            .tool
            .render_diff(&file.diff_content, terminal_width)
            .unwrap_or_else(|_| file.diff_content.clone());

        let lines = Arc::new(parse_ansi_to_lines(&rendered));

        self.cached_result = Some((file_path, content_hash, Arc::clone(&lines)));

        lines.as_ref().clone()
    }
}

fn parse_ansi_to_lines(ansi_text: &str) -> Vec<Line<'static>> {
    match ansi_to_tui::IntoText::into_text(&ansi_text) {
        Ok(text) => text.lines,
        Err(_) => ansi_text
            .lines()
            .map(|line| {
                let color = if line.starts_with('+') {
                    GREEN
                } else if line.starts_with('-') {
                    RED
                } else if line.starts_with("@@") {
                    BLUE
                } else if line.starts_with("diff ")
                    || line.starts_with("index ")
                    || line.starts_with("---")
                    || line.starts_with("+++")
                {
                    SUBTEXT0
                } else {
                    TEXT
                };

                Line::from(Span::styled(line.to_string(), Style::default().fg(color)))
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use christina_core::GitFileStatus;
    use compact_str::CompactString;

    fn create_test_file(path: &str, diff_content: &str) -> GitFile {
        GitFile {
            path: path.into(),
            status: CompactString::new("M"),
            status_enum: GitFileStatus::Modified,
            diff_content: diff_content.to_string(),
            is_binary: false,
        }
    }

    #[test]
    fn test_render_binary_file() {
        let mut renderer = DiffRenderer::new(DiffTool::Basic);
        let mut file = create_test_file("image.png", "");
        file.is_binary = true;

        let lines = renderer.render_diff(&file, 80);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_render_empty_diff() {
        let mut renderer = DiffRenderer::new(DiffTool::Basic);
        let file = create_test_file("file.txt", "");

        let lines = renderer.render_diff(&file, 80);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_basic_diff_rendering() {
        let mut renderer = DiffRenderer::new(DiffTool::Basic);
        let diff = "@@ -1,3 +1,3 @@\n context\n-removed\n+added\n";
        let file = create_test_file("file.txt", diff);

        let lines = renderer.render_diff(&file, 80);
        assert!(lines.len() >= 4);
    }

    #[test]
    fn test_cache_hit_same_file() {
        let mut renderer = DiffRenderer::new(DiffTool::Basic);
        let file = create_test_file("file.txt", "test content");

        let lines1 = renderer.render_diff(&file, 80);
        let lines2 = renderer.render_diff(&file, 80);

        assert_eq!(lines1.len(), lines2.len());
    }

    #[test]
    fn test_cache_miss_different_content() {
        let mut renderer = DiffRenderer::new(DiffTool::Basic);
        let file1 = create_test_file("file.txt", "content1");
        let file2 = create_test_file("file.txt", "content2");

        let _lines1 = renderer.render_diff(&file1, 80);
        let _lines2 = renderer.render_diff(&file2, 80);

        assert!(renderer.cached_result.is_some());
    }
}
