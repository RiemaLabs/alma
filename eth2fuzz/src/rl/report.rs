use failure::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RunManifest {
    pub run_id: String,
    pub filter: String,
    pub mode: String,
    pub fuzzer: Option<String>,
    pub total_seconds: i32,
    pub segment_seconds: i32,
    pub threads: Option<i32>,
    pub beacon_api: Option<String>,
    pub prom_endpoint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CorporaSummary {
    pub counts: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SegmentRecord {
    pub index: usize,
    pub target: String,
    pub bin: Option<String>,
    pub batch_count: usize,
    pub reward: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_cov_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hfuzz_units_delta: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mutated_new: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mutated_new_cov: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PruneRecord {
    pub target: String,
    pub corpora_label: String,
    pub merged_count: usize,
    pub kept_count: usize,
    pub out_dir: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RunStats {
    pub manifest: RunManifest,
    pub initial_corpora: CorporaSummary,
    pub segments: Vec<SegmentRecord>,
    pub prunes: Vec<PruneRecord>,
}

pub struct RunPaths {
    pub root: PathBuf,
    pub outputs_root: PathBuf,
    pub segments_dir: PathBuf,
    pub stats_file: PathBuf,
}

pub fn init_run_paths(workspace: &Path, run_id: &str) -> Result<RunPaths, Error> {
    let root = workspace.join("rl_runs").join(run_id);
    let outputs_root = root.join("outputs");
    let segments_dir = root.join("segments");
    fs::create_dir_all(&outputs_root)?;
    fs::create_dir_all(&segments_dir)?;
    let stats_file = root.join("stats.json");
    Ok(RunPaths { root, outputs_root, segments_dir, stats_file })
}

pub fn summarize_corpora(workspace: &Path) -> Result<CorporaSummary, Error> {
    let mut counts = BTreeMap::new();
    let base = workspace.join("corpora");
    if base.exists() {
        for e in fs::read_dir(base)? {
            let p = e?.path();
            if p.is_dir() {
                let label = p.file_name().unwrap().to_string_lossy().to_string();
                let mut c = 0usize;
                for f in fs::read_dir(&p)? { if f?.path().is_file() { c+=1; } }
                counts.insert(label, c);
            }
        }
    }
    Ok(CorporaSummary { counts })
}

pub fn load_or_init_stats(paths: &RunPaths, manifest: RunManifest, init_summary: CorporaSummary) -> Result<RunStats, Error> {
    if paths.stats_file.exists() {
        let s = fs::read_to_string(&paths.stats_file)?;
        let st: RunStats = serde_json::from_str(&s)?;
        return Ok(st);
    }
    let stats = RunStats { manifest, initial_corpora: init_summary, segments: vec![], prunes: vec![] };
    store_stats(paths, &stats)?;
    Ok(stats)
}

pub fn store_stats(paths: &RunPaths, stats: &RunStats) -> Result<(), Error> {
    let s = serde_json::to_string_pretty(stats)?;
    fs::write(&paths.stats_file, s)?;
    Ok(())
}
