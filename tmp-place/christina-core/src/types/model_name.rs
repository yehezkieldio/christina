use compact_str::CompactString;
use serde::{Deserialize, Serialize};

/// LLM model identifier.
///
/// WHY CompactString: Model names like "gpt-4", "claude-3-opus" are typically short
/// (5-20 bytes) and benefit from inline storage. This is a frequently cloned type
/// in the LLM orchestration pipeline, so avoiding heap allocation reduces pressure
/// on the allocator.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelName(CompactString);

#[cfg(feature = "jsonschema")]
impl schemars::JsonSchema for ModelName {
    fn schema_name() -> String {
        "ModelName".to_string()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            metadata: Some(Box::new(schemars::schema::Metadata {
                description: Some("LLM model identifier (e.g., 'gpt-4', 'claude-3-opus')".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

impl ModelName {
    pub fn name(name: impl Into<CompactString>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for ModelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ModelName {
    fn from(s: String) -> Self {
        Self::name(s)
    }
}

impl From<&str> for ModelName {
    fn from(s: &str) -> Self {
        Self::name(s)
    }
}

impl AsRef<str> for ModelName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_name_from_str() {
        let model = ModelName::from("gpt-5");
        assert_eq!(model.as_ref(), "gpt-5");
    }

    #[test]
    fn model_name_equality() {
        let m1 = ModelName::from("claude-sonnet-4.5");
        let m2 = ModelName::from("claude-sonnet-4.5");
        assert_eq!(m1, m2);
    }

    #[test]
    fn model_name_display() {
        let model = ModelName::from("gpt-5.2-codex");
        assert_eq!(format!("{}", model), "gpt-5.2-codex");
    }

    #[test]
    fn model_name_clone() {
        let m1 = ModelName::from("kimi-k2.5");
        let m2 = m1.clone();
        assert_eq!(m1, m2);
    }
}
