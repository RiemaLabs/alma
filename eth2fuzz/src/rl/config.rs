use failure::Error;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RLConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_exploration")]
    pub exploration_rate: f64,
    #[serde(default = "default_ucb_c")]
    pub ucb_c: f64,
    #[serde(default = "default_use_bins")]
    pub use_bins: bool,
    #[serde(default = "default_save_policy")]
    pub save_policy: bool,
    #[serde(default)]
    pub policy_path: Option<String>,
    #[serde(default)]
    pub load_policy_path: Option<String>,
}

fn default_mode() -> String {
    "Bandit".to_string()
}
fn default_exploration() -> f64 {
    0.2
}
fn default_ucb_c() -> f64 {
    // Use the standard library constant to avoid clippy::approx_constant
    std::f64::consts::SQRT_2
}
fn default_use_bins() -> bool {
    true
}
fn default_save_policy() -> bool {
    true
}

impl Default for RLConfig {
    fn default() -> Self {
        RLConfig {
            enabled: false,
            mode: default_mode(),
            exploration_rate: default_exploration(),
            ucb_c: default_ucb_c(),
            use_bins: default_use_bins(),
            save_policy: default_save_policy(),
            policy_path: None,
            load_policy_path: None,
        }
    }
}

impl RLConfig {
    pub fn from_file(path: &str) -> Result<Self, Error> {
        let s = fs::read_to_string(path)?;
        let cfg: RLConfig = serde_json::from_str(&s)?;
        Ok(cfg)
    }
}
