use std::{num::NonZeroU32, str::FromStr};

use serde::{Deserialize, Serialize};

pub const MAX_INPUT: u32 = 256_000;
pub const MAX_OUTPUT: u32 = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct TokenCount(NonZeroU32);

impl TokenCount {
    pub fn new(count: u32) -> Option<Self> {
        NonZeroU32::new(count).map(Self)
    }

    pub fn new_saturating(value: u32) -> Self {
        NonZeroU32::new(value)
            .map(Self)
            .unwrap_or(Self(NonZeroU32::MIN))
    }

    pub fn try_from_usize(value: usize) -> Result<Self, String> {
        u32::try_from(value)
            .map_err(|_| "Value out of range for u32".to_string())
            .and_then(Self::try_from)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl TryFrom<u32> for TokenCount {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value).ok_or_else(|| "Token count must be greater than zero".to_string())
    }
}

impl From<TokenCount> for u32 {
    fn from(value: TokenCount) -> Self {
        value.get()
    }
}

impl From<TokenCount> for usize {
    fn from(value: TokenCount) -> Self {
        value.get() as usize
    }
}

impl FromStr for TokenCount {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value: u32 = s.parse().map_err(|_| "Invalid token count".to_string())?;
        Self::try_from(value)
    }
}
