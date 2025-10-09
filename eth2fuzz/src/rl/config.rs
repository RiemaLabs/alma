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
}

fn default_mode() -> String { "Bandit".to_string() }
fn default_exploration() -> f64 { 0.2 }
fn default_ucb_c() -> f64 { 1.41421356237 }
fn default_use_bins() -> bool { true }

impl Default for RLConfig {
    fn default() -> Self {
        RLConfig {
            enabled: false,
            mode: default_mode(),
            exploration_rate: default_exploration(),
            ucb_c: default_ucb_c(),
            use_bins: default_use_bins(),
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

