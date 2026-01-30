use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Boolean,
    Number { min: Option<i64>, max: Option<i64> },
    Secret,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub key: String,
    pub label: String,
    pub help: String,
    pub field_type: FieldType,
    pub required: bool,
    pub read_only: bool,
}

impl FieldDef {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        let key = key.into();
        Self {
            key: key.clone(),
            label: label.into(),
            help: String::new(),
            field_type: FieldType::Text,
            required: false,
            read_only: false,
        }
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    pub fn field_type(mut self, field_type: FieldType) -> Self {
        self.field_type = field_type;
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

pub trait Editable {
    fn fields(&self) -> Vec<FieldDef>;
    fn get_field(&self, key: &str) -> Option<String>;
    fn set_field(&mut self, key: &str, value: &str) -> Result<()>;

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}
