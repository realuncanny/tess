use serde::{Deserialize, Serialize};

pub mod sidebar;
pub mod state;
pub mod theme;
pub mod timezone;

pub const MIN_SCALE: f64 = 0.8;
pub const MAX_SCALE: f64 = 1.5;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct ScaleFactor(f64);

impl Default for ScaleFactor {
    fn default() -> Self {
        Self(1.0)
    }
}

impl From<f64> for ScaleFactor {
    fn from(value: f64) -> Self {
        ScaleFactor(value.clamp(MIN_SCALE, MAX_SCALE))
    }
}

impl From<ScaleFactor> for f64 {
    fn from(value: ScaleFactor) -> Self {
        value.0
    }
}
