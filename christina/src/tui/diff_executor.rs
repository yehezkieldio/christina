use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::config::DiffTool;

#[derive(Debug, Clone)]
pub enum ResolvedDiffTool {
    Delta { path: PathBuf },
    DiffSoFancy { path: PathBuf },
    Git,
    Basic,
}

impl ResolvedDiffTool {
    pub fn detect_from(tool: DiffTool) -> Self {
        match tool {
            DiffTool::Auto => Self::auto_detect(),
            DiffTool::Delta => Self::find_delta().unwrap_or_else(|_| Self::auto_detect()),
            DiffTool::DiffSoFancy => {
                Self::find_diff_so_fancy().unwrap_or_else(|_| Self::auto_detect())
            }
            DiffTool::Git => Self::Git,
            DiffTool::Basic => Self::Basic,
        }
    }

    fn auto_detect() -> Self {
        if let Ok(tool) = Self::find_delta() {
            return tool;
        }
        if let Ok(tool) = Self::find_diff_so_fancy() {
            return tool;
        }
        if which::which("git").is_ok() {
            return Self::Git;
        }
        Self::Basic
    }

    fn find_delta() -> Result<Self> {
        let path = which::which("delta").context("delta not found in PATH")?;
        Ok(Self::Delta { path })
    }

    fn find_diff_so_fancy() -> Result<Self> {
        let path = which::which("diff-so-fancy").context("diff-so-fancy not found in PATH")?;
        Ok(Self::DiffSoFancy { path })
    }

    pub fn render_diff(&self, diff_content: &str, terminal_width: u16) -> Result<String> {
        match self {
            Self::Delta { path } => render_with_delta(path, diff_content, terminal_width),
            Self::DiffSoFancy { path } => render_with_diff_so_fancy(path, diff_content),
            Self::Git => render_with_git(diff_content),
            Self::Basic => Ok(diff_content.to_string()),
        }
    }
}

fn render_with_delta(delta_path: &PathBuf, diff_content: &str, width: u16) -> Result<String> {
    let mut child = Command::new(delta_path)
        .arg("--paging=never")
        .arg("--no-gitconfig")
        .arg(format!("--width={}", width))
        .arg("--tabs=4")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn delta process")?;

    if let Some(mut stdin) = child.stdin.take() {
        let diff_bytes = diff_content.as_bytes();
        if diff_bytes.len() > 65536 {
            let owned_bytes = diff_bytes.to_vec();
            std::thread::spawn(move || {
                let _ = stdin.write_all(&owned_bytes);
            });
        } else {
            stdin
                .write_all(diff_bytes)
                .context("Failed to write to delta stdin")?;
        }
    }

    let output = child
        .wait_with_output()
        .context("Failed to wait for delta process")?;

    if !output.status.success() {
        anyhow::bail!("Delta exited with non-zero status: {}", output.status);
    }

    let output_str = String::from_utf8(output.stdout).context("Delta output is not valid UTF-8")?;

    // Strip git diff header lines that would duplicate the file path shown in the tab title.
    // Delta preserves these lines from the input diff, but they're redundant in the dashboard.
    let filtered_lines: Vec<&str> = output_str
        .lines()
        .filter(|line| {
            !line.starts_with("diff --git")
                && !line.starts_with("index ")
                && !line.starts_with("--- ")
                && !line.starts_with("+++ ")
                && !line.starts_with("similarity index")
                && !line.starts_with("rename from")
                && !line.starts_with("rename to")
                && !line.starts_with("copy from")
                && !line.starts_with("copy to")
        })
        .collect();

    Ok(filtered_lines.join("\n"))
}

fn render_with_diff_so_fancy(dff_path: &PathBuf, diff_content: &str) -> Result<String> {
    let mut child = Command::new(dff_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn diff-so-fancy process")?;

    if let Some(mut stdin) = child.stdin.take() {
        let diff_bytes = diff_content.as_bytes();
        if diff_bytes.len() > 65536 {
            let owned_bytes = diff_bytes.to_vec();
            std::thread::spawn(move || {
                let _ = stdin.write_all(&owned_bytes);
            });
        } else {
            stdin
                .write_all(diff_bytes)
                .context("Failed to write to diff-so-fancy stdin")?;
        }
    }

    let output = child
        .wait_with_output()
        .context("Failed to wait for diff-so-fancy process")?;

    if !output.status.success() {
        anyhow::bail!(
            "diff-so-fancy exited with non-zero status: {}",
            output.status
        );
    }

    String::from_utf8(output.stdout).context("diff-so-fancy output is not valid UTF-8")
}

fn render_with_git(diff_content: &str) -> Result<String> {
    let git_path = which::which("git").context("git not found in PATH")?;

    let mut child = Command::new(git_path)
        .arg("diff")
        .arg("--color=always")
        .arg("--no-index")
        .arg("/dev/null")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn git process")?;

    if let Some(mut stdin) = child.stdin.take() {
        let diff_bytes = diff_content.as_bytes();
        if diff_bytes.len() > 65536 {
            let owned_bytes = diff_bytes.to_vec();
            std::thread::spawn(move || {
                let _ = stdin.write_all(&owned_bytes);
            });
        } else {
            stdin
                .write_all(diff_bytes)
                .context("Failed to write to git stdin")?;
        }
    }

    let output = child
        .wait_with_output()
        .context("Failed to wait for git process")?;

    String::from_utf8(output.stdout).context("Git output is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_detect_finds_something() {
        let tool = ResolvedDiffTool::auto_detect();
        matches!(
            tool,
            ResolvedDiffTool::Delta { .. }
                | ResolvedDiffTool::DiffSoFancy { .. }
                | ResolvedDiffTool::Git
                | ResolvedDiffTool::Basic
        );
    }

    #[test]
    fn test_basic_fallback_always_works() {
        let tool = ResolvedDiffTool::Basic;
        let input = "some diff content";
        let result = tool.render_diff(input, 80).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_detect_from_basic() {
        let tool = ResolvedDiffTool::detect_from(DiffTool::Basic);
        assert!(matches!(tool, ResolvedDiffTool::Basic));
    }
}
