use anyhow::{Context, Result};
use std::io::{self, BufRead, Write};

use crate::cli::ProfileCommands;
use crate::config::Config;
use crate::tui::{run_profile_tui, ProfileTuiOptions};
use christina_core::config::{Secret, SecretRef};
use christina_core::profile::ProviderProfile;
use christina_core::types::{ModelName, ProviderKind, TokenCount};

trait ConfigStore {
    #[allow(dead_code)]
    fn load(&mut self) -> Result<Config>;
    #[allow(dead_code)]
    fn save(&mut self, config: &Config) -> Result<()>;
}

struct GlobalConfigStore;

impl ConfigStore for GlobalConfigStore {
    fn load(&mut self) -> Result<Config> {
        Config::load()
    }

    fn save(&mut self, config: &Config) -> Result<()> {
        config.save_to_global()
    }
}

fn parse_secret_input(key: &str) -> Secret<String> {
    match SecretRef::parse(key) {
        Ok(SecretRef::EnvVar(name)) => Secret::EnvVar(name),
        Ok(SecretRef::Keyring(key_name)) => Secret::Keyring(key_name),
        Ok(SecretRef::Literal(value)) => Secret::Value(value),
        Err(_) => Secret::Value(key.to_string()),
    }
}

/// Handle profile commands - routes between CLI and TUI based on subcommand.
pub fn handle_profile_command(command: ProfileCommands) -> Result<()> {
    let mut store = GlobalConfigStore;
    let mut input = io::BufReader::new(io::stdin());
    let mut output = io::stdout();
    handle_profile_command_with_deps(command, &mut store, &mut input, &mut output)
}

fn handle_profile_command_with_deps(
    command: ProfileCommands,
    store: &mut dyn ConfigStore,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<()> {
    match command {
        ProfileCommands::List => handle_list(store, output),
        ProfileCommands::Show { name } => handle_show(store, output, &name),
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
            store,
            output,
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
            store,
            output,
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
        ProfileCommands::Delete { name, force } => {
            handle_delete(store, input, output, &name, force)
        }
        ProfileCommands::Switch { name } => handle_switch(store, output, &name),
        ProfileCommands::Duplicate { source, new_name } => {
            handle_duplicate(store, output, &source, &new_name)
        }
        ProfileCommands::Tui => handle_tui(),
    }
}

fn handle_list(store: &mut dyn ConfigStore, output: &mut dyn Write) -> Result<()> {
    let config = store.load()?;

    let profiles = config.profiles.list_names();

    if profiles.is_empty() {
        writeln!(output, "No profiles configured.")?;
        writeln!(
            output,
            "Use 'christina profile create <name>' to create one."
        )?;
    } else {
        writeln!(output, "Profiles:")?;
        for name in &profiles {
            let marker = if config.profiles.active.as_ref() == Some(name) {
                " *"
            } else {
                ""
            };
            writeln!(output, "  {}{}", name, marker)?;
        }
        if config.profiles.active.is_none() {
            writeln!(output, "\nNo active profile set.")?;
        }
    }

    Ok(())
}

fn handle_show(store: &mut dyn ConfigStore, output: &mut dyn Write, name: &str) -> Result<()> {
    let config = store.load()?;

    match config.profiles.get(name) {
        Some(profile) => {
            writeln!(output, "Profile: {}", profile.name)?;
            writeln!(output, "  Provider: {}", profile.provider)?;
            writeln!(output, "  Model: {}", profile.model)?;
            let api_key_display = match &profile.api_key {
                Secret::Value(_) => "<set>".to_string(),
                Secret::EnvVar(name) => format!("<env:{}>", name),
                Secret::Keyring(key) => format!("<keyring:{}>", key),
            };
            writeln!(output, "  API Key: {}", api_key_display)?;
            writeln!(
                output,
                "  API URL: {}",
                profile
                    .api_url
                    .as_ref()
                    .map(|u| u.as_str())
                    .unwrap_or("<not set>")
            )?;
            writeln!(
                output,
                "  Max Input Tokens: {}",
                profile.max_input_tokens.get()
            )?;
            writeln!(
                output,
                "  Max Output Tokens: {}",
                profile.max_output_tokens.get()
            )?;
            writeln!(
                output,
                "  Azure API Version: {}",
                profile
                    .azure_api_version
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| "<not set>".to_string())
            )?;
            writeln!(
                output,
                "  Azure Deployment ID: {}",
                profile
                    .azure_deployment_id
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| "<not set>".to_string())
            )?;

            if config.profiles.active.as_deref() == Some(name) {
                writeln!(output, "\n  [Active Profile]")?;
            }
        }
        None => {
            anyhow::bail!("Profile '{}' not found", name);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_create(
    store: &mut dyn ConfigStore,
    output: &mut dyn Write,
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
    let mut config = store.load()?;

    if config.profiles.exists(name) {
        anyhow::bail!("Profile '{}' already exists", name);
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
        profile.api_key = parse_secret_input(&key);
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
    store.save(&config)?;

    writeln!(output, "Created profile: {}", name)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_edit(
    store: &mut dyn ConfigStore,
    output: &mut dyn Write,
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
    let mut config = store.load()?;

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
        profile.api_key = parse_secret_input(&key);
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
    store.save(&config)?;

    writeln!(output, "Updated profile: {}", name)?;

    Ok(())
}

fn handle_delete(
    store: &mut dyn ConfigStore,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    name: &str,
    force: bool,
) -> Result<()> {
    let mut config = store.load()?;

    if !config.profiles.exists(name) {
        anyhow::bail!("Profile '{}' not found", name);
    }

    // Confirm deletion unless --force
    if !force {
        write!(output, "Delete profile '{}' ? [y/N]: ", name)?;
        output.flush()?;

        let mut response = String::new();
        input.read_line(&mut response)?;

        if !response.trim().eq_ignore_ascii_case("y") {
            writeln!(output, "Cancelled.")?;
            return Ok(());
        }
    }

    config.profiles.remove(name)?;
    store.save(&config)?;

    writeln!(output, "Deleted profile: {}", name)?;

    Ok(())
}

fn handle_switch(store: &mut dyn ConfigStore, output: &mut dyn Write, name: &str) -> Result<()> {
    let mut config = store.load()?;

    config.profiles.set_active(name)?;

    // Apply the profile to current config
    if let Some(profile) = config.profiles.get(name).cloned() {
        config.apply_profile(&profile);
    }

    store.save(&config)?;

    writeln!(output, "Switched to profile: {}", name)?;

    Ok(())
}

fn handle_duplicate(
    store: &mut dyn ConfigStore,
    output: &mut dyn Write,
    source: &str,
    new_name: &str,
) -> Result<()> {
    let mut config = store.load()?;

    if !config.profiles.exists(source) {
        anyhow::bail!("Source profile '{}' not found", source);
    }

    if config.profiles.exists(new_name) {
        anyhow::bail!("Profile '{}' already exists", new_name);
    }

    let source_profile = config
        .profiles
        .get(source)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Source profile '{}' not found", source))?;
    let mut new_profile = source_profile;
    new_profile.name = new_name.to_string();

    config.profiles.add(new_profile)?;
    store.save(&config)?;

    writeln!(output, "Duplicated '{}' to '{}'", source, new_name)?;

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
