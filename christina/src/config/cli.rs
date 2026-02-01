use anyhow::Result;

use crate::cli::ConfigCommands;
use crate::config::Config;
use crate::tui::{
    run_config_tui, run_profile_tui, ConfigTuiOptions, ConfigTuiResult, ProfileTuiOptions,
};

/// Handle config commands - routes between CLI and TUI based on subcommand.
pub fn handle_config_command(command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Get { key } => handle_get(&key),
        ConfigCommands::Set { key, value } => handle_set(&key, &value),
        ConfigCommands::List => handle_list(),
        ConfigCommands::Path => {
            handle_path();
            Ok(())
        }
        ConfigCommands::Tui => handle_tui(),
    }
}

fn handle_get(key: &str) -> Result<()> {
    let config = Config::load()?;

    match config.get(key) {
        Some(value) => {
            if key.contains("api_key") || key.contains("key") {
                println!("{}: <hidden>", key);
            } else {
                println!("{}: {}", key, value);
            }
        }
        None => {
            eprintln!("Error: Unknown configuration key '{}'", key);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn handle_set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;

    config.set(key, value)?;
    config.save_to_global()?;

    println!("Set {} = {}", key, value);

    Ok(())
}

fn handle_list() -> Result<()> {
    let config = Config::load()?;

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
            christina_core::types::commit_message::ValidationMode::Strict => "strict",
            christina_core::types::commit_message::ValidationMode::Soft => "soft",
            christina_core::types::commit_message::ValidationMode::Disabled => "disabled",
        }
    );
    println!("  diff_tool: {}", config.diff.tool);
    println!("  diff_show_preview: {}", config.diff.show_preview);
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

    Ok(())
}

fn handle_path() {
    match Config::global_config_path() {
        Some(path) => println!("{}", path.display()),
        None => {
            eprintln!("Error: Could not determine config directory");
            std::process::exit(1);
        }
    }
}

fn handle_tui() -> Result<()> {
    // Load config once before entering the loop
    let mut config = Config::load().unwrap_or_default();

    loop {
        let has_api_key = config.api_key.is_some();
        let api_key_source = Some("config/env");

        let options = ConfigTuiOptions {
            config: config.clone(),
            has_api_key,
            api_key_source,
            on_save: Box::new(move |cfg| cfg.clone().save_to_global()),
        };

        match run_config_tui(options)? {
            ConfigTuiResult::Quit => break,
            ConfigTuiResult::OpenProfiles => {
                // Open profiles with current config
                manage_profiles(&config)?;
                // Reload config after profile manager exits
                config = Config::load().unwrap_or_default();
            }
        }
    }

    Ok(())
}

fn manage_profiles(config: &Config) -> Result<()> {
    let options = ProfileTuiOptions {
        profiles: config.profiles.clone(),
        active_profile: config.profiles.active.clone(),
        on_save: Box::new(|profiles_manager| {
            let mut cfg = Config::load().unwrap_or_default();
            cfg.profiles = profiles_manager.clone();

            // Apply active profile if set
            if let Some(active) = profiles_manager.get_active() {
                cfg.apply_profile(active);
            }

            cfg.save_to_global()
        }),
    };

    run_profile_tui(options)
}
