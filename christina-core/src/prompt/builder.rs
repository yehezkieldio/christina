//! Prompt composition from diff content, summaries, and themes.

use super::templates::{
    DIRECT_COMMIT_PROMPT, INTENT_EXTRACTION_PROMPT, SUMMARY_PROMPT, SYSTEM_PROMPT,
    THEME_SYNTHESIS_PROMPT, USER_CONTEXT_MAX_LEN, USER_CONTEXT_TEMPLATE,
};

#[derive(Debug, Clone)]
pub struct Theme {
    pub title: String,
    pub description: String,
    pub file_count: usize,
    pub scope: Option<String>,
}

impl Theme {
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        file_count: usize,
        scope: Option<String>,
    ) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            file_count,
            scope,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PromptBuilder<'a> {
    diff: Option<&'a str>,
    user_context: Option<&'a str>,
    summaries: &'a [String],
    themes: &'a [Theme],
}

impl<'a> PromptBuilder<'a> {
    pub fn new() -> Self {
        Self {
            diff: None,
            user_context: None,
            summaries: &[],
            themes: &[],
        }
    }

    pub fn with_diff(mut self, diff: &'a str) -> Self {
        self.diff = Some(diff);
        self
    }

    pub fn with_user_context(mut self, ctx: &'a str) -> Self {
        let ctx = ctx.trim();
        if !ctx.is_empty() {
            // Keep user context bounded so prompts stay within token budgets.
            let truncated = if ctx.len() > USER_CONTEXT_MAX_LEN {
                let mut end = USER_CONTEXT_MAX_LEN;
                while end > 0 && !ctx.is_char_boundary(end) {
                    end -= 1;
                }
                &ctx[..end]
            } else {
                ctx
            };
            self.user_context = Some(truncated);
        }
        self
    }

    pub fn with_summaries(mut self, summaries: &'a [String]) -> Self {
        self.summaries = summaries;
        self
    }

    pub fn with_themes(mut self, themes: &'a [Theme]) -> Self {
        self.themes = themes;
        self
    }

    pub fn build_system_prompt(&self) -> String {
        SYSTEM_PROMPT.to_string()
    }

    pub fn build_summary_prompt(&self) -> String {
        let diff = self.diff.unwrap_or("");
        // Pre-allocate to avoid reallocation when large diffs are injected.
        let estimated_capacity = SUMMARY_PROMPT.len() + diff.len() + 20;
        let mut prompt = String::with_capacity(estimated_capacity);

        if let Some(placeholder_pos) = SUMMARY_PROMPT.find("{diff}") {
            prompt.push_str(&SUMMARY_PROMPT[..placeholder_pos]);
            prompt.push_str("```diff\n");
            prompt.push_str(diff);
            prompt.push_str("\n```");
            prompt.push_str(&SUMMARY_PROMPT[placeholder_pos + 6..]);
        } else {
            prompt.push_str(SUMMARY_PROMPT);
        }

        prompt
    }

    pub fn build_intent_prompt(&self) -> String {
        // Estimate list size so we can build summaries without repeated growth.
        let estimated_summaries_size = self.summaries.len() * 80;
        let estimated_capacity = INTENT_EXTRACTION_PROMPT.len() + estimated_summaries_size;
        let mut summaries_text = String::with_capacity(estimated_summaries_size);

        for (i, summary) in self.summaries.iter().enumerate() {
            if i > 0 {
                summaries_text.push('\n');
            }
            use std::fmt::Write;
            let _ = write!(summaries_text, "{}. {}", i + 1, summary);
        }

        let mut prompt = String::with_capacity(estimated_capacity);
        if let Some(placeholder_pos) = INTENT_EXTRACTION_PROMPT.find("{summaries}") {
            prompt.push_str(&INTENT_EXTRACTION_PROMPT[..placeholder_pos]);
            prompt.push_str(&summaries_text);
            prompt.push_str(&INTENT_EXTRACTION_PROMPT[placeholder_pos + 11..]);
        } else {
            prompt.push_str(INTENT_EXTRACTION_PROMPT);
        }

        prompt
    }

    pub fn build_synthesis_prompt(&self) -> String {
        let estimated_themes_size = self.themes.len() * 100;
        let estimated_context_size = self
            .user_context
            .map_or(0, |ctx| ctx.len() + USER_CONTEXT_TEMPLATE.len());
        let estimated_capacity =
            THEME_SYNTHESIS_PROMPT.len() + estimated_themes_size + estimated_context_size;

        let mut themes_text = String::with_capacity(estimated_themes_size);

        for (i, theme) in self.themes.iter().enumerate() {
            if i > 0 {
                themes_text.push('\n');
            }

            use std::fmt::Write;
            match &theme.scope {
                Some(scope) => {
                    let _ = write!(
                        themes_text,
                        "- {} ({}): {} [{} files]",
                        theme.title, scope, theme.description, theme.file_count
                    );
                }
                None => {
                    let _ = write!(
                        themes_text,
                        "- {}: {} [{} files]",
                        theme.title, theme.description, theme.file_count
                    );
                }
            }
        }

        let mut prompt = String::with_capacity(estimated_capacity);
        if let Some(placeholder_pos) = THEME_SYNTHESIS_PROMPT.find("{themes}") {
            prompt.push_str(&THEME_SYNTHESIS_PROMPT[..placeholder_pos]);
            prompt.push_str(&themes_text);
            prompt.push_str(&THEME_SYNTHESIS_PROMPT[placeholder_pos + 8..]);
        } else {
            prompt.push_str(THEME_SYNTHESIS_PROMPT);
        }

        if let Some(ctx) = self.user_context
            && let Some(context_pos) = USER_CONTEXT_TEMPLATE.find("{context}")
        {
            prompt.push_str(&USER_CONTEXT_TEMPLATE[..context_pos]);
            prompt.push_str(ctx);
            prompt.push_str(&USER_CONTEXT_TEMPLATE[context_pos + 9..]);
        }

        prompt
    }

    pub fn build_direct_prompt(&self) -> String {
        let diff = self.diff.unwrap_or("");
        let estimated_context_size = self
            .user_context
            .map_or(0, |ctx| ctx.len() + USER_CONTEXT_TEMPLATE.len());
        let estimated_capacity =
            DIRECT_COMMIT_PROMPT.len() + diff.len() + 20 + estimated_context_size;
        let mut prompt = String::with_capacity(estimated_capacity);

        if let Some(placeholder_pos) = DIRECT_COMMIT_PROMPT.find("{diff}") {
            prompt.push_str(&DIRECT_COMMIT_PROMPT[..placeholder_pos]);
            prompt.push_str("```diff\n");
            prompt.push_str(diff);
            prompt.push_str("\n```");
            prompt.push_str(&DIRECT_COMMIT_PROMPT[placeholder_pos + 6..]);
        } else {
            prompt.push_str(DIRECT_COMMIT_PROMPT);
        }

        if let Some(ctx) = self.user_context
            && let Some(context_pos) = USER_CONTEXT_TEMPLATE.find("{context}")
        {
            prompt.push_str(&USER_CONTEXT_TEMPLATE[..context_pos]);
            prompt.push_str(ctx);
            prompt.push_str(&USER_CONTEXT_TEMPLATE[context_pos + 9..]);
        }

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_format() {
        let builder = PromptBuilder::new();
        let prompt = builder.build_system_prompt();
        assert!(prompt.contains("type(scope): description"));
        assert!(!prompt.contains("ADDITIONAL CONSTRAINT"));
    }

    #[test]
    fn system_prompt_conventional_commits() {
        let builder = PromptBuilder::new();
        let prompt = builder.build_system_prompt();
        assert!(prompt.contains("Conventional Commits"));
    }

    #[test]
    fn system_prompt_has_anti_slop() {
        let builder = PromptBuilder::new();
        let prompt = builder.build_system_prompt();
        assert!(prompt.contains("VERBOTEN VOCABULARY"));
        assert!(prompt.contains("BANNED VERBS"));
        assert!(prompt.contains("VERB TIERS"));
    }

    #[test]
    fn system_prompt_has_explicit_types() {
        let builder = PromptBuilder::new();
        let prompt = builder.build_system_prompt();
        for ty in &[
            "feat", "fix", "refactor", "chore", "style", "perf", "docs", "build", "test", "ci",
            "security", "compat", "i18n",
        ] {
            assert!(prompt.contains(ty), "missing type: {ty}");
        }
    }

    #[test]
    fn system_prompt_has_workspace_scope_rules() {
        let builder = PromptBuilder::new();
        let prompt = builder.build_system_prompt();
        assert!(prompt.contains("SCOPE RULES"));
        assert!(prompt.contains("WORKSPACE-LEVEL SCOPE DETECTION"));
        assert!(prompt.contains("INFRASTRUCTURE SCOPE"));
    }

    #[test]
    fn summary_prompt() {
        let builder = PromptBuilder::new().with_diff("diff --git a/foo.rs");
        let prompt = builder.build_summary_prompt();
        assert!(prompt.contains("```diff"));
        assert!(prompt.contains("diff --git a/foo.rs"));
        assert!(prompt.contains("SINGLE sentence"));
        assert!(prompt.contains("ANTI-SLOP"));
    }

    #[test]
    fn intent_prompt() {
        let summaries = vec![
            "Added user authentication".to_string(),
            "Fixed login bug".to_string(),
        ];
        let builder = PromptBuilder::new().with_summaries(&summaries);
        let prompt = builder.build_intent_prompt();
        assert!(prompt.contains("1. Added user authentication"));
        assert!(prompt.contains("2. Fixed login bug"));
    }

    #[test]
    fn synthesis_prompt() {
        let themes = vec![Theme::new(
            "Authentication",
            "Add user login flow",
            5,
            Some("auth".to_string()),
        )];
        let builder = PromptBuilder::new().with_themes(&themes);
        let prompt = builder.build_synthesis_prompt();
        assert!(prompt.contains("Authentication (auth)"));
        assert!(prompt.contains("[5 files]"));
    }

    #[test]
    fn synthesis_prompt_without_scope() {
        let themes = vec![Theme::new(
            "Cross-cutting refactor",
            "Refactor logging across multiple modules",
            8,
            None,
        )];
        let builder = PromptBuilder::new().with_themes(&themes);
        let prompt = builder.build_synthesis_prompt();
        assert!(prompt.contains("Cross-cutting refactor:"));
        assert!(!prompt.contains("()"));
        assert!(prompt.contains("[8 files]"));
    }

    #[test]
    fn direct_prompt_with_context() {
        let builder = PromptBuilder::new()
            .with_diff("some diff")
            .with_user_context("This fixes issue #123");
        let prompt = builder.build_direct_prompt();
        assert!(prompt.contains("some diff"));
        assert!(prompt.contains("This fixes issue #123"));
    }

    #[test]
    fn user_context_truncates() {
        let long_context = "x".repeat(USER_CONTEXT_MAX_LEN + 10);
        let builder = PromptBuilder::new()
            .with_diff("some diff")
            .with_user_context(&long_context);
        let prompt = builder.build_direct_prompt();
        assert!(prompt.contains("some diff"));
        assert!(!prompt.contains(&long_context));
        assert!(prompt.contains(&"x".repeat(USER_CONTEXT_MAX_LEN)));
    }
}
