use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DiffTool {
    #[default]
    /// Auto-detect available tool (delta > diff-so-fancy > git > basic)
    Auto,
    /// Use delta (https://github.com/dandavison/delta)
    Delta,
    /// Use diff-so-fancy (https://github.com/so-fancy/diff-so-fancy)
    DiffSoFancy,
    /// Use git diff --color=always
    Git,
    /// Basic fallback with no external tools
    Basic,
}

impl fmt::Display for DiffTool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Delta => write!(f, "delta"),
            Self::DiffSoFancy => write!(f, "diff-so-fancy"),
            Self::Git => write!(f, "git"),
            Self::Basic => write!(f, "basic"),
        }
    }
}

impl std::str::FromStr for DiffTool {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "delta" => Ok(Self::Delta),
            "diff-so-fancy" | "diffsofancy" => Ok(Self::DiffSoFancy),
            "git" => Ok(Self::Git),
            "basic" | "builtin" | "none" => Ok(Self::Basic),
            _ => Err(format!(
                "Unknown diff tool '{}'. Valid options: auto, delta, diff-so-fancy, git, basic",
                s
            )),
        }
    }
}

/// Diff tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DiffConfig {
    pub tool: DiffTool,
    pub show_preview: bool,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            tool: DiffTool::Auto,
            show_preview: true,
        }
    }
}

impl DiffConfig {
    pub fn from_env() -> Option<Self> {
        std::env::var("CHRISTINA_DIFF_TOOL")
            .ok()
            .and_then(|s| s.parse::<DiffTool>().ok())
            .map(|tool| Self {
                tool,
                show_preview: true,
            })
    }

    pub fn with_env_override(mut self) -> Self {
        if let Some(env_config) = Self::from_env() {
            self.tool = env_config.tool;
        }
        if let Ok(env_val) = std::env::var("CHRISTINA_DIFF_SHOW_PREVIEW")
            && let Ok(v) = env_val.parse()
        {
            self.show_preview = v;
        }
        self
    }
}
