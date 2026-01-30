/// Macro to generate a string wrapper newtype with common trait implementations.
///
/// This macro reduces boilerplate for string newtypes by automatically implementing
/// standard traits: Display, From<String>, From<&str>, AsRef<str>, and optionally
/// additional traits based on configuration.
///
/// # Basic Usage
///
/// ```rust
/// string_wrapper! {
///     pub struct ModelName(CompactString);
/// }
/// ```
///
/// # With Serde Support
///
/// ```rust
/// string_wrapper! {
///     #[derive(Serialize, Deserialize)]
///     #[serde(transparent)]
///     pub struct FilePath(CompactString);
/// }
/// ```
///
/// # With Validation
///
/// ```rust
/// string_wrapper! {
///     #[validate(|s| !s.is_empty(), "cannot be empty")]
///     pub struct CommitMessage(String);
/// }
/// ```
#[macro_export]
macro_rules! string_wrapper {
    // Pattern 1: With serde and other derives
    (
        $(#[derive($($derives:ident),*)])?
        $(#[serde($($serde_meta:tt)*)])?
        $(#[validate($validator:expr)])?
        $vis:vis struct $name:ident($inner:ty);
    ) => {
        $(#[derive($($derives),*)])?
        $(#[serde($($serde_meta)*)])?
        $vis struct $name($inner);

        impl $name {
            /// Create a new instance.
            pub fn new(value: impl Into<$inner>) -> Self {
                Self(value.into())
            }

            /// Get the string content as a string slice.
            pub fn as_str(&self) -> &str {
                self.0.as_ref()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self::new(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self::new(s)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        $(
            impl $name {
                pub fn validate(value: &$inner) -> Result<(), String> {
                    ($validator)(value)
                }
            }
        )?
    };
}

/// Macro to generate a string wrapper that validates on construction.
#[macro_export]
macro_rules! validated_string_wrapper {
    (
        $(#[derive($($derives:ident),*)])?
        $(#[serde($($serde_meta:tt)*)])?
        $vis:vis struct $name:ident($inner:ty);
        validate: |$param:ident| $condition:expr => $error:expr;
    ) => {
        $(#[derive($($derives),*)])?
        $(#[serde($($serde_meta)*)])?
        $vis struct $name($inner);

        impl $name {
            /// Create a new validated instance.
            pub fn new(value: impl Into<$inner>) -> Result<Self, String> {
                let inner: $inner = value.into();
                let $param = &inner;
                if !$condition {
                    return Err($error);
                }
                Ok(Self(inner))
            }

            /// Get the string content as a string slice.
            pub fn as_str(&self) -> &str {
                self.0.as_ref()
            }
        }

        impl TryFrom<String> for $name {
            type Error = String;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::new(s)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = String;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                Self::new(s.to_string())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use compact_str::CompactString;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestModel(CompactString);

    impl TestModel {
        fn new(value: impl Into<CompactString>) -> Self {
            Self(value.into())
        }

        fn as_str(&self) -> &str {
            self.0.as_str()
        }
    }

    impl std::fmt::Display for TestModel {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.as_str())
        }
    }

    impl From<String> for TestModel {
        fn from(s: String) -> Self {
            Self::new(s)
        }
    }

    impl From<&str> for TestModel {
        fn from(s: &str) -> Self {
            Self::new(s)
        }
    }

    impl AsRef<str> for TestModel {
        fn as_ref(&self) -> &str {
            self.as_str()
        }
    }

    impl std::ops::Deref for TestModel {
        type Target = str;

        fn deref(&self) -> &Self::Target {
            self.as_str()
        }
    }

    #[test]
    fn test_basic_wrapper() {
        let model = TestModel::from("gpt-4");
        assert_eq!(model.as_str(), "gpt-4");
        assert_eq!(format!("{}", model), "gpt-4");

        let as_str: &str = model.as_ref();
        assert_eq!(as_str, "gpt-4");

        let deref: &str = &model;
        assert_eq!(deref, "gpt-4");
    }

    #[test]
    fn test_from_string() {
        let s = "claude-3".to_string();
        let model = TestModel::from(s);
        assert_eq!(model.as_str(), "claude-3");
    }
}
