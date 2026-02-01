use std::convert::TryFrom;
use thiserror::Error;
use url::Url;

/// Azure OpenAI endpoint configuration parsed from a URL.
///
/// This newtype wraps the parsed components of an Azure OpenAI URL:
/// - endpoint: The base URL (scheme + host)
/// - deployment_id: The deployment name (e.g., "gpt-4")
/// - api_version: The API version to use (defaults to "2024-12-01-preview")
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzureEndpoint {
    pub endpoint: String,
    pub deployment_id: String,
    pub api_version: String,
}

/// Error types for Azure endpoint parsing.
#[derive(Debug, Error)]
pub enum AzureEndpointError {
    #[error("Invalid Azure URL: {0}")]
    InvalidUrl(String),
    #[error("Not an Azure OpenAI endpoint")]
    NotAzureEndpoint,
    #[error("Missing deployment ID in URL path")]
    MissingDeploymentId,
}

impl TryFrom<Url> for AzureEndpoint {
    type Error = AzureEndpointError;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        // Check if this is an Azure endpoint
        if !url.host_str().is_some_and(|host| {
            host.contains("cognitiveservices.azure.com") || host.contains("openai.azure.com")
        }) {
            return Err(AzureEndpointError::NotAzureEndpoint);
        }

        // Extract endpoint (scheme + host)
        let endpoint = format!(
            "{}://{}",
            url.scheme(),
            url.host_str().ok_or(AzureEndpointError::InvalidUrl(
                "Unable to extract host".to_string()
            ))?
        );

        // Extract deployment_id from path "/openai/deployments/{id}"
        let path = url.path();
        let deployment_id = path
            .strip_prefix("/openai/deployments/")
            .ok_or(AzureEndpointError::MissingDeploymentId)?
            .split('/')
            .next()
            .ok_or(AzureEndpointError::MissingDeploymentId)?
            .to_string();

        if deployment_id.is_empty() {
            return Err(AzureEndpointError::MissingDeploymentId);
        }

        // Extract api-version from query params (default: "2024-12-01-preview")
        let api_version = url
            .query_pairs()
            .find(|(key, _)| key == "api-version")
            .map(|(_, value)| value.to_string())
            .unwrap_or_else(|| "2024-12-01-preview".to_string());

        Ok(AzureEndpoint {
            endpoint,
            deployment_id,
            api_version,
        })
    }
}

impl TryFrom<&str> for AzureEndpoint {
    type Error = AzureEndpointError;

    fn try_from(url_str: &str) -> Result<Self, Self::Error> {
        let url = Url::parse(url_str).map_err(|e| AzureEndpointError::InvalidUrl(e.to_string()))?;
        url.try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_azure_url_full() {
        let url = "https://myresource.cognitiveservices.azure.com/openai/deployments/gpt-4/chat/completions?api-version=2024-12-01-preview";
        let endpoint: AzureEndpoint = url.try_into().expect("Valid Azure URL should parse");

        assert_eq!(
            endpoint.endpoint,
            "https://myresource.cognitiveservices.azure.com"
        );
        assert_eq!(endpoint.deployment_id, "gpt-4");
        assert_eq!(endpoint.api_version, "2024-12-01-preview");
    }

    #[test]
    fn test_parse_azure_url_openai_azure_domain() {
        let url = "https://myresource.openai.azure.com/openai/deployments/gpt-4.1-mini/chat/completions?api-version=2025-01-01";
        let endpoint: AzureEndpoint = url.try_into().expect("Valid Azure OpenAI URL should parse");

        assert_eq!(endpoint.endpoint, "https://myresource.openai.azure.com");
        assert_eq!(endpoint.deployment_id, "gpt-4.1-mini");
        assert_eq!(endpoint.api_version, "2025-01-01");
    }

    #[test]
    fn test_parse_non_azure_url() {
        let url = "https://api.openai.com/v1/chat/completions";
        let result: Result<AzureEndpoint, _> = url.try_into();

        assert!(result.is_err());
        assert!(matches!(result, Err(AzureEndpointError::NotAzureEndpoint)));
    }

    #[test]
    fn test_parse_azure_url_defaults_api_version() {
        let url = "https://myresource.cognitiveservices.azure.com/openai/deployments/gpt-4/chat/completions";
        let endpoint: AzureEndpoint = url
            .try_into()
            .expect("Azure URL without api-version should parse");

        assert_eq!(endpoint.api_version, "2024-12-01-preview");
    }

    #[test]
    fn test_parse_azure_url_invalid_url() {
        let url = "not a valid url";
        let result: Result<AzureEndpoint, _> = url.try_into();

        assert!(result.is_err());
        assert!(matches!(result, Err(AzureEndpointError::InvalidUrl(_))));
    }

    #[test]
    fn test_parse_azure_url_missing_deployment() {
        let url = "https://myresource.cognitiveservices.azure.com/other/path";
        let result: Result<AzureEndpoint, _> = url.try_into();

        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(AzureEndpointError::MissingDeploymentId)
        ));
    }

    #[test]
    fn test_parse_azure_url_empty_deployment() {
        let url = "https://myresource.cognitiveservices.azure.com/openai/deployments/";
        let result: Result<AzureEndpoint, _> = url.try_into();

        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(AzureEndpointError::MissingDeploymentId)
        ));
    }

    #[test]
    fn test_try_from_url_object() {
        let url = Url::parse("https://myresource.cognitiveservices.azure.com/openai/deployments/gpt-4/chat/completions?api-version=2024-12-01-preview")
            .expect("Valid URL");
        let endpoint: AzureEndpoint = url.try_into().expect("Valid Azure URL should parse");

        assert_eq!(
            endpoint.endpoint,
            "https://myresource.cognitiveservices.azure.com"
        );
        assert_eq!(endpoint.deployment_id, "gpt-4");
        assert_eq!(endpoint.api_version, "2024-12-01-preview");
    }
}
