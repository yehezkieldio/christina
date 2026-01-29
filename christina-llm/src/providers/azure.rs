use url::Url;

#[derive(Debug, Clone)]
pub struct ParsedAzureConfig {
    pub endpoint: String,
    pub deployment_id: String,
    pub api_version: String,
}

pub fn parse_azure_url(url: &str) -> Option<ParsedAzureConfig> {
    if !url.contains("cognitiveservices.azure.com") && !url.contains("openai.azure.com") {
        return None;
    }

    let url_parsed = Url::parse(url).ok()?;
    let endpoint = format!("{}://{}", url_parsed.scheme(), url_parsed.host_str()?);

    let path = url_parsed.path();
    let deployment_id = path
        .strip_prefix("/openai/deployments/")?
        .split('/')
        .next()?
        .to_string();

    if deployment_id.is_empty() {
        return None;
    }

    let api_version = url_parsed
        .query_pairs()
        .find(|(key, _)| key == "api-version")
        .map(|(_, value): (_, std::borrow::Cow<'_, str>)| value.to_string())
        .unwrap_or_else(|| "2024-12-01-preview".to_string());

    Some(ParsedAzureConfig {
        endpoint,
        deployment_id,
        api_version,
    })
}
