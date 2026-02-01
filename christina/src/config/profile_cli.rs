use anyhow::{Context, Result};

use crate::cli::ProfileCommands;
use crate::config::Config;
use crate::tui::{run_profile_tui, ProfileTuiOptions};
use christina_core::config::Secret;
use christina_core::profile::ProviderProfile;
use christina_core::types::{ModelName, ProviderKind, TokenCount};

/// Handle profile commands - routes between CLI and TUI based on subcommand.
pub fn handle_profile_command(command: ProfileCommands) -> Result<()> {
    match command {
        ProfileCommands::List => handle_list(),
        ProfileCommands::Show { name } => handle_show(&name),
        ProfileCommands::Create {
            name,
            provider,
            model,
            api_key,
            api_url,
            max_input_tokens,
            max_output_tokens,
            azure_api_version,
            azure_deployment_id,
        } => handle_create(
            &name,
            provider,
            model,
            api_key,
            api_url,
            max_input_tokens,
            max_output_tokens,
            azure_api_version,
            azure_deployment_id,
        ),
        ProfileCommands::Edit {
            name,
            provider,
            model,
            api_key,
            api_url,
            max_input_tokens,
            max_output_tokens,
            azure_api_version,
            azure_deployment_id,
        } => handle_edit(
            &name,
            provider,
            model,
            api_key,
            api_url,
            max_input_tokens,
            max_output_tokens,
            azure_api_version,
            azure_deployment_id,
        ),
        ProfileCommands::Delete { name, force } => handle_delete(&name, force),
        ProfileCommands::Switch { name } => handle_switch(&name),
        ProfileCommands::Duplicate { source, new_name } => handle_duplicate(&source, &new_name),
        ProfileCommands::Tui => handle_tui(),
    }
}

fn handle_list() -> Result<()> {
    let config = Config::load()?;

    let profiles = config.profiles.list_names();

    if profiles.is_empty() {
        println!("No profiles configured.");
        println!("Use 'christina profile create <name>' to create one.");
    } else {
        println!("Profiles:");
        for name in &profiles {
            let marker = if config.profiles.active.as_ref() == Some(name) {
                " *"
            } else {
                ""
            };
            println!("  {}{}", name, marker);
        }
        if config.profiles.active.is_none() {
            println!("\nNo active profile set.");
        }
    }

    Ok(())
}

fn handle_show(name: &str) -> Result<()> {
    let config = Config::load()?;

    match config.profiles.get(name) {
        Some(profile) => {
            println!("Profile: {}", profile.name);
            println!("  Provider: {}", profile.provider);
            println!("  Model: {}", profile.model);
            let api_key_display = match &profile.api_key {
                Secret::Value(_) => "<set>".to_string(),
                Secret::EnvVar(name) => format!("<env:{}>", name),
                Secret::Keyring(key) => format!("<keyring:{}>", key),
            };
            println!("  API Key: {}", api_key_display);
            println!(
                "  API URL: {}",
                profile
                    .api_url
                    .as_ref()
                    .map(|u| u.as_str())
                    .unwrap_or("<not set>")
            );
            println!("  Max Input Tokens: {}", profile.max_input_tokens.get());
            println!("  Max Output Tokens: {}", profile.max_output_tokens.get());
            println!(
                "  Azure API Version: {}",
                profile
                    .azure_api_version
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| "<not set>".to_string())
            );
            println!(
                "  Azure Deployment ID: {}",
                profile
                    .azure_deployment_id
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| "<not set>".to_string())
            );

            if config.profiles.active.as_deref() == Some(name) {
                println!("\n  [Active Profile]");
            }
        }
        None => {
            eprintln!("Error: Profile '{}' not found", name);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_create(
    name: &str,
    provider: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    api_url: Option<String>,
    max_input_tokens: Option<usize>,
    max_output_tokens: Option<usize>,
    azure_api_version: Option<String>,
    azure_deployment_id: Option<String>,
) -> Result<()> {
    let mut config = Config::load()?;

    if config.profiles.exists(name) {
        eprintln!("Error: Profile '{}' already exists", name);
        std::process::exit(1);
    }

    // Parse provider with fallback
    let provider_kind = match provider {
        Some(p) => p.parse().map_err(anyhow::Error::msg)?,
        None => ProviderKind::OpenAI,
    };

    // Create base profile
    let mut profile = ProviderProfile::new(
        name.to_string(),
        provider_kind,
        model
            .map(ModelName::from)
            .unwrap_or_else(|| ModelName::from("gpt-4.1-mini")),
    );

    // Apply optional fields
    if let Some(key) = api_key {
        profile.api_key = Secret::Value(key);
    }

    if let Some(url) = api_url {
        profile.api_url = Some(url.parse().context("Invalid API URL")?);
    }

    if let Some(tokens) = max_input_tokens {
        profile.max_input_tokens = TokenCount::new_saturating(tokens as u32);
    }

    if let Some(tokens) = max_output_tokens {
        profile.max_output_tokens = TokenCount::new_saturating(tokens as u32);
    }

    if let Some(version) = azure_api_version {
        profile.azure_api_version = Some(version);
    }

    if let Some(deployment) = azure_deployment_id {
        profile.azure_deployment_id = Some(deployment);
    }

    // Validate and add
    profile.validate().context("Profile validation failed")?;
    config.profiles.add(profile)?;
    config.save_to_global()?;

    println!("Created profile: {}", name);

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_edit(
    name: &str,
    provider: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    api_url: Option<String>,
    max_input_tokens: Option<usize>,
    max_output_tokens: Option<usize>,
    azure_api_version: Option<String>,
    azure_deployment_id: Option<String>,
) -> Result<()> {
    let mut config = Config::load()?;

    let mut profile = config
        .profiles
        .get(name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))?;

    // Apply optional fields
    if let Some(p) = provider {
        profile.provider = p.parse().map_err(anyhow::Error::msg)?;
    }

    if let Some(m) = model {
        profile.model = ModelName::from(m);
    }

    if let Some(key) = api_key {
        profile.api_key = Secret::Value(key);
    }

    if let Some(url) = api_url {
        profile.api_url = Some(url.parse().context("Invalid API URL")?);
    }

    if let Some(tokens) = max_input_tokens {
        profile.max_input_tokens = TokenCount::new_saturating(tokens as u32);
    }

    if let Some(tokens) = max_output_tokens {
        profile.max_output_tokens = TokenCount::new_saturating(tokens as u32);
    }

    if let Some(version) = azure_api_version {
        profile.azure_api_version = Some(version);
    }

    if let Some(deployment) = azure_deployment_id {
        profile.azure_deployment_id = Some(deployment);
    }

    // Validate and update
    profile.validate().context("Profile validation failed")?;
    config.profiles.update(name, profile)?;
    config.save_to_global()?;

    println!("Updated profile: {}", name);

    Ok(())
}

fn handle_delete(name: &str, force: bool) -> Result<()> {
    let mut config = Config::load()?;

    if !config.profiles.exists(name) {
        eprintln!("Error: Profile '{}' not found", name);
        std::process::exit(1);
    }

    // Confirm deletion unless --force
    if !force {
        print!("Delete profile '{}' ? [y/N]: ", name);
        use std::io::Write;
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    config.profiles.remove(name)?;
    config.save_to_global()?;

    println!("Deleted profile: {}", name);

    Ok(())
}

fn handle_switch(name: &str) -> Result<()> {
    let mut config = Config::load()?;

    config.profiles.set_active(name)?;

    // Apply the profile to current config
    if let Some(profile) = config.profiles.get(name).cloned() {
        config.apply_profile(&profile);
    }

    config.save_to_global()?;

    println!("Switched to profile: {}", name);

    Ok(())
}

fn handle_duplicate(source: &str, new_name: &str) -> Result<()> {
    let mut config = Config::load()?;

    if !config.profiles.exists(source) {
        eprintln!("Error: Source profile '{}' not found", source);
        std::process::exit(1);
    }

    if config.profiles.exists(new_name) {
        eprintln!("Error: Profile '{}' already exists", new_name);
        std::process::exit(1);
    }

    let source_profile = config
        .profiles
        .get(source)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Source profile '{}' not found", source))?;
    let mut new_profile = source_profile;
    new_profile.name = new_name.to_string();

    config.profiles.add(new_profile)?;
    config.save_to_global()?;

    println!("Duplicated '{}' to '{}'", source, new_name);

    Ok(())
}

fn handle_tui() -> Result<()> {
    let config = Config::load().unwrap_or_default();

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
