use serde::{Deserialize, Serialize};

/// Validated temperature value for LLM sampling (0.0 - 2.0).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Temperature(f32);

impl Temperature {
    pub const MIN: f32 = 0.0;
    pub const MAX: f32 = 2.0;

    /// Create a validated temperature.
    pub fn try_new(value: f32) -> Result<Self, String> {
        if value.is_nan() {
            return Err("Temperature must be a valid number".to_string());
        }
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(format!(
                "Temperature must be between {} and {}, got {}",
                Self::MIN,
                Self::MAX,
                value
            ));
        }
        Ok(Self(value))
    }

    /// Create a temperature by clamping into the valid range.
    /// NaN values are coerced to MIN.
    pub fn new_clamped(value: f32) -> Self {
        if value.is_nan() {
            return Self(Self::MIN);
        }
        Self(value.clamp(Self::MIN, Self::MAX))
    }

    pub fn value(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Temperature {
    type Error = String;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<Temperature> for f32 {
    fn from(value: Temperature) -> Self {
        value.0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn temperature_valid() {
        let temp = Temperature::try_new(0.7).unwrap();
        assert_eq!(temp.value(), 0.7);
    }

    #[test]
    fn temperature_invalid_nan() {
        assert!(Temperature::try_new(f32::NAN).is_err());
    }

    #[test]
    fn temperature_invalid_low() {
        assert!(Temperature::try_new(-0.1).is_err());
    }

    #[test]
    fn temperature_invalid_high() {
        assert!(Temperature::try_new(3.0).is_err());
    }

    #[test]
    fn temperature_clamped() {
        assert_eq!(Temperature::new_clamped(-1.0).value(), 0.0);
        assert_eq!(Temperature::new_clamped(3.0).value(), 2.0);
    }
}
