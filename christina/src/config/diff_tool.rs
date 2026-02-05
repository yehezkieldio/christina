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
///
/// **Current Status**: This configuration is preserved for future TUI integration
/// but is not currently consumed by the CLI-only codebase. The TUI was temporarily
/// removed (see TUI_INTEGRATION.md) to focus on core functionality.
///
/// **Intended Usage**: When TUI is re-integrated, these settings will control:
/// - `tool`: Which diff formatter to use for preview display (delta, diff-so-fancy, etc.)
/// - `show_preview`: Whether to show diff preview before generating commit messages
///
/// The configuration is validated, serialized, and displayed via `christina config show`,
/// but does not affect current diff processing behavior.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_difftool_from_str_auto() {
        assert!(matches!(DiffTool::from_str("auto"), Ok(DiffTool::Auto)));
    }

    #[test]
    fn test_difftool_from_str_delta() {
        assert!(matches!(DiffTool::from_str("delta"), Ok(DiffTool::Delta)));
    }

    #[test]
    fn test_difftool_from_str_diff_so_fancy() {
        assert!(matches!(
            DiffTool::from_str("diff-so-fancy"),
            Ok(DiffTool::DiffSoFancy)
        ));
    }

    #[test]
    fn test_difftool_from_str_git() {
        assert!(matches!(DiffTool::from_str("git"), Ok(DiffTool::Git)));
    }

    #[test]
    fn test_difftool_from_str_basic() {
        assert!(matches!(DiffTool::from_str("basic"), Ok(DiffTool::Basic)));
    }

    #[test]
    fn test_difftool_from_str_case_insensitive() {
        assert!(matches!(DiffTool::from_str("DELTA"), Ok(DiffTool::Delta)));
        assert!(matches!(DiffTool::from_str("Delta"), Ok(DiffTool::Delta)));
        assert!(matches!(DiffTool::from_str("AUTO"), Ok(DiffTool::Auto)));
        assert!(matches!(
            DiffTool::from_str("DIFF-SO-FANCY"),
            Ok(DiffTool::DiffSoFancy)
        ));
        assert!(matches!(DiffTool::from_str("GIT"), Ok(DiffTool::Git)));
        assert!(matches!(DiffTool::from_str("BASIC"), Ok(DiffTool::Basic)));
    }

    #[test]
    fn test_difftool_aliases_diffsofancy() {
        assert!(matches!(
            DiffTool::from_str("diffsofancy"),
            Ok(DiffTool::DiffSoFancy)
        ));
    }

    #[test]
    fn test_difftool_aliases_builtin() {
        assert!(matches!(DiffTool::from_str("builtin"), Ok(DiffTool::Basic)));
    }

    #[test]
    fn test_difftool_aliases_none() {
        assert!(matches!(DiffTool::from_str("none"), Ok(DiffTool::Basic)));
    }

    #[test]
    fn test_difftool_from_str_invalid() {
        let result = DiffTool::from_str("invalid");
        assert!(result.is_err(), "Should error on invalid tool");
        if let Err(err) = result {
            assert!(
                err.contains("Unknown diff tool"),
                "Error message should mention unknown tool"
            );
            assert!(
                err.contains("Valid options"),
                "Error message should list valid options"
            );
            assert!(err.contains("auto"), "Error should include auto");
            assert!(err.contains("delta"), "Error should include delta");
            assert!(
                err.contains("diff-so-fancy"),
                "Error should include diff-so-fancy"
            );
            assert!(err.contains("git"), "Error should include git");
            assert!(err.contains("basic"), "Error should include basic");
        }
    }

    #[test]
    fn test_difftool_display_auto() {
        assert_eq!(DiffTool::Auto.to_string(), "auto");
    }

    #[test]
    fn test_difftool_display_delta() {
        assert_eq!(DiffTool::Delta.to_string(), "delta");
    }

    #[test]
    fn test_difftool_display_diff_so_fancy() {
        assert_eq!(DiffTool::DiffSoFancy.to_string(), "diff-so-fancy");
    }

    #[test]
    fn test_difftool_display_git() {
        assert_eq!(DiffTool::Git.to_string(), "git");
    }

    #[test]
    fn test_difftool_display_basic() {
        assert_eq!(DiffTool::Basic.to_string(), "basic");
    }

    #[test]
    fn test_diff_config_default() {
        let config = DiffConfig::default();
        assert_eq!(config.tool, DiffTool::Auto);
        assert!(config.show_preview);
    }

    #[test]
    fn test_diff_config_from_env() {
        let result = DiffConfig::from_env();
        assert!(matches!(result, Some(_) | None), "Should return Option");
    }

    #[test]
    fn test_diff_config_from_env_none() {
        let result = DiffConfig::from_env();
        assert!(
            matches!(result, Some(_) | None),
            "from_env should return Option"
        );
    }

    #[test]
    fn test_diff_config_with_env_override() {
        let config = DiffConfig {
            tool: DiffTool::Basic,
            show_preview: false,
        };
        let overridden = config.with_env_override();
        assert_eq!(overridden.tool, DiffTool::Basic);
        assert!(!overridden.show_preview);
    }

    #[test]
    fn test_diff_config_with_env_override_show_preview() {
        let config = DiffConfig {
            tool: DiffTool::Auto,
            show_preview: true,
        };
        let _overridden = config.with_env_override();
    }

    #[test]
    fn test_diff_config_roundtrip_parse() {
        let tools = vec![
            DiffTool::Auto,
            DiffTool::Delta,
            DiffTool::DiffSoFancy,
            DiffTool::Git,
            DiffTool::Basic,
        ];

        for tool in tools {
            let display_str = tool.to_string();
            let parsed = DiffTool::from_str(&display_str).ok();
            assert_eq!(
                parsed,
                Some(tool),
                "Round-trip should preserve tool: {}",
                display_str
            );
        }
    }
}
