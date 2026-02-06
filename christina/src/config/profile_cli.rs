use anyhow::{Context, Result};
use std::io::{self, BufRead, Write};

use crate::cli::ProfileCommands;
use crate::config::Config;
use christina_core::config::{Secret, SecretRef};
use christina_core::profile::ProviderProfile;
use christina_core::types::{ModelName, ProviderKind, TokenCount};

trait ConfigStore {
    fn load(&mut self) -> Result<Config>;
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

fn parse_secret_input(key: &str, allow_plaintext: bool) -> Result<Secret<String>> {
    match SecretRef::parse(key).map_err(anyhow::Error::msg)? {
        SecretRef::EnvVar(name) => Ok(Secret::EnvVar(name)),
        SecretRef::Keyring(key_name) => Ok(Secret::Keyring(key_name)),
        SecretRef::Literal(value) => {
            if !allow_plaintext {
                tracing::warn!(
                    "Storing plaintext API key. Consider env:VAR_NAME or keyring:KEY_NAME."
                );
                eprintln!(
                    "Warning: storing plaintext API key. Consider env:VAR_NAME or keyring:KEY_NAME."
                );
            }
            Ok(Secret::Value(value))
        }
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
            allow_plaintext,
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
            allow_plaintext,
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
            allow_plaintext,
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
            allow_plaintext,
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
    allow_plaintext: bool,
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
        profile.api_key = parse_secret_input(&key, allow_plaintext)?;
    }

    if let Some(url) = api_url {
        profile.api_url = Some(url.parse().context("Invalid API URL")?);
    }

    if let Some(tokens) = max_input_tokens {
        profile.max_input_tokens = TokenCount::new_at_least_one(tokens as u32);
    }

    if let Some(tokens) = max_output_tokens {
        profile.max_output_tokens = TokenCount::new_at_least_one(tokens as u32);
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
    allow_plaintext: bool,
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
        profile.api_key = parse_secret_input(&key, allow_plaintext)?;
    }

    if let Some(url) = api_url {
        profile.api_url = Some(url.parse().context("Invalid API URL")?);
    }

    if let Some(tokens) = max_input_tokens {
        profile.max_input_tokens = TokenCount::new_at_least_one(tokens as u32);
    }

    if let Some(tokens) = max_output_tokens {
        profile.max_output_tokens = TokenCount::new_at_least_one(tokens as u32);
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

#[cfg(test)]
mod tests {
    use super::*;
    use christina_core::profile::{Profiles, ProviderProfile};
    use christina_core::types::{ModelName, ProviderKind};
    use std::io::{BufReader, Cursor};

    struct MockConfigStore {
        config: Config,
        save_called: bool,
    }

    impl MockConfigStore {
        fn new() -> Self {
            Self {
                config: Config {
                    profiles: Profiles::new(),
                    ..Default::default()
                },
                save_called: false,
            }
        }

        fn with_profile(mut self, name: &str) -> Self {
            let profile = ProviderProfile::new(
                name.to_string(),
                ProviderKind::OpenAI,
                ModelName::from("gpt-4"),
            );
            self.config.profiles.add(profile).unwrap();
            self
        }

        fn with_active(mut self, name: &str) -> Self {
            self.config.profiles.set_active(name).unwrap();
            self
        }
    }

    impl ConfigStore for MockConfigStore {
        fn load(&mut self) -> Result<Config> {
            Ok(self.config.clone())
        }

        fn save(&mut self, config: &Config) -> Result<()> {
            self.config = config.clone();
            self.save_called = true;
            Ok(())
        }
    }

    #[test]
    fn test_list_empty() {
        let mut store = MockConfigStore::new();
        let mut output = Vec::new();

        handle_list(&mut store, &mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("No profiles configured"));
    }

    #[test]
    fn test_list_with_profiles() {
        let mut store = MockConfigStore::new()
            .with_profile("dev")
            .with_profile("prod")
            .with_active("dev");
        let mut output = Vec::new();

        handle_list(&mut store, &mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("dev *"));
        assert!(output_str.contains("prod"));
    }

    #[test]
    fn test_show_existing_profile() {
        let mut store = MockConfigStore::new().with_profile("test");
        let mut output = Vec::new();

        handle_show(&mut store, &mut output, "test").unwrap();

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Profile: test"));
        assert!(output_str.contains("Provider: openai"));
    }

    #[test]
    fn test_show_nonexistent_profile() {
        let mut store = MockConfigStore::new();
        let mut output = Vec::new();

        let result = handle_show(&mut store, &mut output, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_create_basic_profile() {
        let mut store = MockConfigStore::new();
        let mut output = Vec::new();

        handle_create(
            &mut store,
            &mut output,
            "new",
            Some("openai".to_string()),
            Some("gpt-4".to_string()),
            Some("env:OPENAI_KEY".to_string()),
            false,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert!(store.save_called);
        assert!(store.config.profiles.exists("new"));
        let profile = store.config.profiles.get("new").unwrap();
        assert_eq!(profile.provider, ProviderKind::OpenAI);
    }

    #[test]
    fn test_create_duplicate_profile() {
        let mut store = MockConfigStore::new().with_profile("existing");
        let mut output = Vec::new();

        let result = handle_create(
            &mut store,
            &mut output,
            "existing",
            Some("openai".to_string()),
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_create_with_all_options() {
        let mut store = MockConfigStore::new();
        let mut output = Vec::new();

        handle_create(
            &mut store,
            &mut output,
            "azure",
            Some("azure".to_string()),
            Some("gpt-4".to_string()),
            Some("keyring:azure-key".to_string()),
            false,
            Some("https://test.openai.azure.com".to_string()),
            Some(100000),
            Some(4000),
            Some("2024-12-01-preview".to_string()),
            Some("gpt-4-deployment".to_string()),
        )
        .unwrap();

        let profile = store.config.profiles.get("azure").unwrap();
        assert_eq!(profile.provider, ProviderKind::Azure);
        assert_eq!(profile.max_input_tokens.get(), 100000);
        assert_eq!(
            profile.azure_api_version,
            Some("2024-12-01-preview".to_string())
        );
    }

    #[test]
    fn test_edit_existing_profile() {
        let mut store = MockConfigStore::new().with_profile("test");
        let mut output = Vec::new();

        handle_edit(
            &mut store,
            &mut output,
            "test",
            None,
            Some("gpt-4-turbo".to_string()),
            None,
            false,
            None,
            Some(200000),
            None,
            None,
            None,
        )
        .unwrap();

        let profile = store.config.profiles.get("test").unwrap();
        assert_eq!(profile.model, ModelName::from("gpt-4-turbo"));
        assert_eq!(profile.max_input_tokens.get(), 200000);
    }

    #[test]
    fn test_edit_nonexistent_profile() {
        let mut store = MockConfigStore::new();
        let mut output = Vec::new();

        let result = handle_edit(
            &mut store,
            &mut output,
            "nonexistent",
            None,
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_delete_with_force() {
        let mut store = MockConfigStore::new().with_profile("test");
        let mut input = BufReader::new(Cursor::new(b""));
        let mut output = Vec::new();

        handle_delete(&mut store, &mut input, &mut output, "test", true).unwrap();

        assert!(!store.config.profiles.exists("test"));
        assert!(store.save_called);
    }

    #[test]
    fn test_delete_with_confirmation_yes() {
        let mut store = MockConfigStore::new().with_profile("test");
        let mut input = BufReader::new(Cursor::new(b"y\n"));
        let mut output = Vec::new();

        handle_delete(&mut store, &mut input, &mut output, "test", false).unwrap();

        assert!(!store.config.profiles.exists("test"));
    }

    #[test]
    fn test_delete_with_confirmation_no() {
        let mut store = MockConfigStore::new().with_profile("test");
        let mut input = BufReader::new(Cursor::new(b"n\n"));
        let mut output = Vec::new();

        handle_delete(&mut store, &mut input, &mut output, "test", false).unwrap();

        assert!(store.config.profiles.exists("test"));
        assert!(!store.save_called);
    }

    #[test]
    fn test_delete_nonexistent_profile() {
        let mut store = MockConfigStore::new();
        let mut input = BufReader::new(Cursor::new(b""));
        let mut output = Vec::new();

        let result = handle_delete(&mut store, &mut input, &mut output, "nonexistent", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_switch_profile() {
        let mut store = MockConfigStore::new()
            .with_profile("dev")
            .with_profile("prod");
        let mut output = Vec::new();

        handle_switch(&mut store, &mut output, "prod").unwrap();

        assert_eq!(store.config.profiles.active, Some("prod".to_string()));
        assert!(store.save_called);
    }

    #[test]
    fn test_switch_nonexistent_profile() {
        let mut store = MockConfigStore::new();
        let mut output = Vec::new();

        let result = handle_switch(&mut store, &mut output, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_profile() {
        let mut store = MockConfigStore::new().with_profile("original");
        let mut output = Vec::new();

        handle_duplicate(&mut store, &mut output, "original", "copy").unwrap();

        assert!(store.config.profiles.exists("original"));
        assert!(store.config.profiles.exists("copy"));
        let copy = store.config.profiles.get("copy").unwrap();
        assert_eq!(copy.name, "copy");
    }

    #[test]
    fn test_duplicate_nonexistent_source() {
        let mut store = MockConfigStore::new();
        let mut output = Vec::new();

        let result = handle_duplicate(&mut store, &mut output, "nonexistent", "copy");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_duplicate_to_existing_name() {
        let mut store = MockConfigStore::new()
            .with_profile("source")
            .with_profile("target");
        let mut output = Vec::new();

        let result = handle_duplicate(&mut store, &mut output, "source", "target");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_parse_secret_input_env() {
        let secret = parse_secret_input("env:MY_KEY", false).unwrap();
        assert!(matches!(secret, Secret::EnvVar(_)));
    }

    #[test]
    fn test_parse_secret_input_keyring() {
        let secret = parse_secret_input("keyring:my-key", false).unwrap();
        assert!(matches!(secret, Secret::Keyring(_)));
    }

    #[test]
    fn test_parse_secret_input_literal() {
        let secret = parse_secret_input("sk-1234567890", true).unwrap();
        assert!(matches!(secret, Secret::Value(_)));
    }

    #[test]
    fn test_parse_secret_input_literal_without_override_warns_but_succeeds() {
        let result = parse_secret_input("sk-1234567890", false);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Secret::Value(_)));
    }
}
