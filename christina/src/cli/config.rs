//! CLI handlers for config subcommands.
//!
//! WHY print-only here: keeps all config persistence in `Config` while CLI is
//! responsible only for routing and user-facing output.

use anyhow::Result;

use crate::cli::ConfigCommands;
use crate::config::Config;
use christina_core::types::UsageTier;

/// Handle config commands - routes between CLI and TUI based on subcommand.
pub fn handle_config_command(command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Get { key } => {
            let config = Config::load()?;
            handle_get_with_config(&config, &key)
        }
        ConfigCommands::Set { key, value } => {
            let mut config = Config::load()?;
            handle_set_with_config(&mut config, &key, &value)?;
            config.save_to_global()
        }
        ConfigCommands::List => {
            let config = Config::load()?;
            handle_list_with_config(&config);
            Ok(())
        }
        ConfigCommands::Path => handle_path(),
    }
}

fn handle_get_with_config(config: &Config, key: &str) -> Result<()> {
    match config.get(key) {
        Some(value) => {
            if key.contains("api_key") || key.contains("key") {
                // Redact secrets even for explicit `get` calls.
                println!("{}: <hidden>", key);
            } else {
                println!("{}: {}", key, value);
            }
            Ok(())
        }
        None => {
            anyhow::bail!("Unknown configuration key '{}'", key);
        }
    }
}

fn handle_set_with_config(config: &mut Config, key: &str, value: &str) -> Result<()> {
    config.set(key, value)?;
    println!("Set {} = {}", key, value);
    Ok(())
}

fn handle_list_with_config(config: &Config) {
    println!("Configuration values:");
    println!("  max_input_tokens: {}", config.max_input_tokens.get());
    println!("  max_output_tokens: {}", config.max_output_tokens.get());
    println!("  model_provider: {}", config.model_provider);
    println!("  model: {}", config.model);
    println!(
        "  api_key: {}",
        config
            .api_key
            .as_ref()
            .map(|_| "<set>".to_string())
            .unwrap_or_else(|| "<not set>".to_string())
    );
    println!(
        "  model_api_url: {}",
        config
            .model_api_url
            .as_ref()
            .map(|u| u.to_string())
            .unwrap_or_else(|| "<not set>".to_string())
    );
    println!(
        "  azure_api_version: {}",
        config
            .azure_api_version
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "<not set>".to_string())
    );
    println!(
        "  azure_deployment_id: {}",
        config
            .azure_deployment_id
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "<not set>".to_string())
    );
    println!("  ignore_files: {}", config.ignore_files.join(", "));
    println!(
        "  lockfile_token_limit: {}",
        config.lockfile_token_limit.get()
    );
    println!("  model_temperature: {}", config.model_temperature);
    println!(
        "  commit_message_max_length: {}",
        config
            .commit_message_max_length
            .map(|v| v.to_string())
            .unwrap_or_else(|| "<default (72)>".to_string())
    );
    println!(
        "  commit_message_validation_mode: {}",
        match config.commit_message_validation_mode {
            christina_core::types::commit::ValidationMode::Strict => "strict",
            christina_core::types::commit::ValidationMode::Soft => "soft",
            christina_core::types::commit::ValidationMode::Disabled => "disabled",
        }
    );
    println!("  use_commit_history: {}", config.use_commit_history);
    println!("  commit_history_depth: {}", config.commit_history_depth);
    println!(
        "  max_concurrent_requests: {}",
        config.max_concurrent_requests
    );
    println!(
        "  max_partial_failure_rate: {}",
        config.max_partial_failure_rate
    );
    println!(
        "  prompt_failure_rate_threshold: {}",
        config.prompt_failure_rate_threshold
    );
    println!(
        "  usage_tier: {}",
        match config.usage_tier {
            UsageTier::Standard => "standard",
            UsageTier::Free => "free",
        }
    );
    println!("  use_experimental: {}", config.use_experimental);
    println!(
        "  free_tier_max_input_tokens: {}",
        config.free_tier.max_input_tokens.get()
    );
    println!(
        "  free_tier_max_output_tokens: {}",
        config.free_tier.max_output_tokens.get()
    );
    println!(
        "  free_tier_max_concurrent_requests: {}",
        config.free_tier.max_concurrent_requests
    );
    println!(
        "  free_tier_commit_history_depth: {}",
        config.free_tier.commit_history_depth
    );
    println!();
    println!(
        "  Active profile: {}",
        config
            .profiles
            .active
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "<none>".to_string())
    );
}

fn handle_path() -> Result<()> {
    match Config::global_config_path() {
        Some(path) => {
            println!("{}", path.display());
            Ok(())
        }
        None => {
            anyhow::bail!("Could not determine config directory");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use christina_core::types::ProviderKind;

    fn create_test_config() -> Config {
        Config {
            model: "gpt-4".into(),
            model_provider: ProviderKind::OpenAI,
            api_key: Some("test-key".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_handle_get_existing_key() {
        let config = create_test_config();
        let result = handle_get_with_config(&config, "model");
        assert!(result.is_ok(), "Should successfully get existing key");
    }

    #[test]
    fn test_handle_get_api_key_hidden() {
        let config = create_test_config();
        let result = handle_get_with_config(&config, "api_key");
        assert!(result.is_ok(), "Should successfully get api_key");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_handle_get_missing_key() {
        let config = create_test_config();
        let result = handle_get_with_config(&config, "nonexistent_key");
        assert!(result.is_err(), "Should error on missing key");
        let error = result.expect_err("Expected error for missing key");
        assert!(
            error.to_string().contains("Unknown configuration key"),
            "Error message should mention unknown key"
        );
    }

    #[test]
    fn test_handle_set_updates_config() {
        let mut config = create_test_config();
        let original_model = config.model.clone();

        let result = handle_set_with_config(&mut config, "model", "gpt-3.5-turbo");
        assert!(result.is_ok(), "Should successfully set config value");

        assert_ne!(config.model, original_model, "Model should be updated");
        assert_eq!(
            config.model.as_str(),
            "gpt-3.5-turbo",
            "Model should be set to new value"
        );
    }

    #[test]
    fn test_handle_set_invalid_key() {
        let mut config = create_test_config();
        let result = handle_set_with_config(&mut config, "invalid_key", "value");
        assert!(result.is_err(), "Should error on invalid key");
    }

    #[test]
    fn test_handle_list_shows_config() {
        let config = create_test_config();
        handle_list_with_config(&config);
    }

    #[test]
    fn test_handle_path_returns_path() {
        let result = handle_path();
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(
                    e.to_string()
                        .contains("Could not determine config directory"),
                    "Error should be about config directory"
                );
            }
        }
    }

    #[test]
    fn test_handle_get_key_with_key_in_name() {
        let config = create_test_config();
        let result = handle_get_with_config(&config, "api_key");
        assert!(result.is_ok(), "Should handle key containing 'key'");
    }

    #[test]
    fn test_handle_set_multiple_times() {
        let mut config = create_test_config();

        let result1 = handle_set_with_config(&mut config, "model", "gpt-3.5-turbo");
        assert!(result1.is_ok());
        assert_eq!(config.model.as_str(), "gpt-3.5-turbo");

        let result2 = handle_set_with_config(&mut config, "model", "gpt-4");
        assert!(result2.is_ok());
        assert_eq!(config.model.as_str(), "gpt-4");
    }

    #[test]
    fn test_handle_set_different_types() {
        let mut config = create_test_config();

        let result = handle_set_with_config(&mut config, "model_provider", "azure");
        assert!(result.is_ok());
        assert_eq!(config.model_provider, ProviderKind::Azure);

        let result = handle_set_with_config(&mut config, "max_input_tokens", "2000");
        assert!(result.is_ok());
        assert_eq!(config.max_input_tokens.get(), 2000);
    }
}
