use crate::env::{corpora_dir, workspace_dir};
use crate::fuzzers::{Fuzzer, FuzzerConfig, FuzzerQuit};
use crate::targets::Targets;
use failure::{bail, Error};
use rand::prelude::*;
use std::collections::HashMap;
use std::env;
use std::time::Instant;
use crate::strum::IntoEnumIterator;

use super::bins::{ensure_size_bins, sample_batch, merge_hfuzz_and_corpora_then_prune_into};
use super::config::RLConfig;
use super::ppo::{PPOPolicy, Transition};
use super::metrics::{MetricsCtx, read_metrics_from_env};
use super::report::{RunManifest, RunPaths, RunStats, init_run_paths, summarize_corpora, load_or_init_stats, store_stats, SegmentRecord, PruneRecord};
use regex::Regex;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct ArmKey {
    target_name: String,
    bin_label: Option<String>,
}

#[derive(Default, Clone)]
struct ArmStat {
    pulls: u64,
    reward_sum: f64,
}

pub struct RLEngine {
    cfg: RLConfig,
    arms: Vec<ArmKey>,
    stats: HashMap<ArmKey, ArmStat>,
    bins: HashMap<String, HashMap<String, String>>, // target -> label -> dir
    rng: StdRng,
    // PPO
    ppo: Option<PPOPolicy>,
    buffer: Vec<Transition>,
    // Latest metrics context features appended to arm features
    last_metrics: MetricsCtx,
    last_features12: Vec<f64>,
    // Reproducibility run context
    run_id: Option<String>,
    run_paths: Option<RunPaths>,
    run_stats: Option<RunStats>,
    seg_index: usize,
    tag: String,
}

impl RLEngine {
    pub fn new(cfg: RLConfig, seed: u64) -> Self {
        Self {
            cfg,
            arms: Vec::new(),
            stats: HashMap::new(),
            bins: HashMap::new(),
            rng: StdRng::seed_from_u64(seed),
            ppo: None,
            buffer: Vec::new(),
            last_metrics: MetricsCtx::default(),
            last_features12: vec![0.0; 12],
            run_id: None,
            run_paths: None,
            run_stats: None,
            seg_index: 0,
            tag: std::env::var("ETH2FUZZ_TAG").unwrap_or_else(|_| "default".to_string()),
        }
    }

    pub fn set_run_id(&mut self, id: String) { self.run_id = Some(id); }
    pub fn set_tag(&mut self, t: String) { self.tag = t; }

    pub fn initialize(&mut self, filter: &str) -> Result<(), Error> {
        // Build list of targets filtered by substring
        let all = crate::targets::get_targets();
        let filtered: Vec<String> = all
            .into_iter()
            .filter(|t| t.contains(filter))
            .collect();
        if filtered.is_empty() {
            bail!(format!("No targets match filter `{}`", filter));
        }

        // Prepare bins per target (if enabled and rust target)
        for name in filtered.iter() {
            let t = Targets::iter().find(|x| x.name() == *name).unwrap();
            if self.cfg.use_bins && t.language() == "rust" {
                let corpus = corpora_dir()?.join(t.corpora());
                let ws = self.logs_root()?.join("rl");
                if corpus.exists() {
                    if let Ok(map) = ensure_size_bins(&t.name(), &corpus, &ws) {
                        let mut string_map = HashMap::new();
                        for (k, v) in map.into_iter() {
                            string_map.insert(k, v.to_string_lossy().to_string());
                        }
                        self.bins.insert(t.name(), string_map);
                    }
                }
            }
        }

        // Build arms: per (target, bin?)
        for name in filtered.into_iter() {
            if let Some(binmap) = self.bins.get(&name) {
                for label in binmap.keys() {
                    self.arms.push(ArmKey {
                        target_name: name.clone(),
                        bin_label: Some(label.clone()),
                    });
                }
            } else {
                self.arms.push(ArmKey {
                    target_name: name,
                    bin_label: None,
                });
            }
        }

        // Initialize PPO if configured
        if self.cfg.mode.to_lowercase().contains("ppo") {
            let feat_dim = self.feature_dim();
            // try load policy if provided
            if let Some(ref path) = self.cfg.load_policy_path {
                if let Ok(s) = std::fs::read_to_string(path) {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                        if let Some(pol) = PPOPolicy::from_json(&v) {
                            self.ppo = Some(pol);
                        }
                    }
                }
            }
            if self.ppo.is_none() {
                self.ppo = Some(PPOPolicy::new(feat_dim, 32, 1e-3, 0.2));
            }
        }

        // Initialize run report if run_id provided
        if let Some(id) = &self.run_id {
            let ws = self.logs_root()?.join("rl");
            let paths = init_run_paths(&ws, id)?;
            let manifest = RunManifest {
                run_id: id.clone(),
                filter: filter.to_string(),
                mode: self.cfg.mode.clone(),
                fuzzer: None,
                total_seconds: 0,
                segment_seconds: 0,
                threads: None,
                beacon_api: std::env::var("ETH2FUZZ_BEACON_API").ok(),
                prom_endpoint: std::env::var("ETH2FUZZ_PROM_ENDPOINT").ok(),
            };
            let init_summary = summarize_corpora(&workspace_dir()?)?;
            let stats = load_or_init_stats(&paths, manifest, init_summary)?;
            self.run_paths = Some(paths);
            self.run_stats = Some(stats);
        }
        Ok(())
    }

    fn select_arm_ucb1(&mut self) -> usize {
        // Pull each arm once until all tried
        let total_pulls: u64 = self.stats.values().map(|s| s.pulls).sum();
        for (i, key) in self.arms.iter().enumerate() {
            if self.stats.get(key).map(|s| s.pulls).unwrap_or(0) == 0 {
                return i;
            }
        }
        // UCB1
        let c = self.cfg.ucb_c;
        let ln_n = (total_pulls as f64).ln().max(1.0);
        let mut best = 0usize;
        let mut best_score = f64::MIN;
        for (i, key) in self.arms.iter().enumerate() {
            let st = self.stats.get(key).cloned().unwrap_or_default();
            let mean = if st.pulls == 0 {
                0.0
            } else {
                st.reward_sum / st.pulls as f64
            };
            let bonus = if st.pulls == 0 {
                f64::INFINITY
            } else {
                c * (ln_n / st.pulls as f64).sqrt()
            };
            let score = mean + bonus;
            if score > best_score {
                best_score = score;
                best = i;
            }
        }
        best
    }

    fn select_arm_ppo(&mut self) -> (usize, Vec<Vec<f64>>, Vec<f64>) {
        let xs = self.build_features_for_all_arms();
        let pol = self.ppo.as_ref().expect("ppo initialized");
        let (_idx0, probs0) = pol.act(&xs);
        // apply context bias on top of policy probs
        let mut biased: Vec<f64> = Vec::with_capacity(self.arms.len());
        for (i, arm) in self.arms.iter().enumerate() {
            let w = self.context_bias_for_arm(arm);
            biased.push((probs0[i] * w).max(1e-6));
        }
        // normalize
        let s: f64 = biased.iter().sum::<f64>().max(1e-9);
        for i in 0..biased.len() { biased[i] /= s; }
        // argmax
        let mut best = 0usize; let mut bestp = biased[0];
        for i in 1..biased.len() { if biased[i] > bestp { best = i; bestp = biased[i]; } }
        (best, xs, biased)
    }

    fn update_stat(&mut self, idx: usize, reward: f64) {
        let key = self.arms[idx].clone();
        let entry = self.stats.entry(key).or_default();
        entry.pulls += 1;
        entry.reward_sum += reward;
    }

    fn run_one_segment(
        &mut self,
        arm: &ArmKey,
        fuzzer: Option<Fuzzer>,
        segment: i32,
        threads: Option<i32>,
    ) -> Result<f64, Error> {
        // Map back to Targets enum
        let target_enum = Targets::iter()
            .find(|x| x.name() == arm.target_name)
            .expect("valid target");
        let mut cfg = FuzzerConfig::default();
        cfg.timeout = Some(segment);
        cfg.thread = threads;

        // Prepare a batch view to run this segment (only rust/hfuzz honors it)
        let prev_override = env::var("ETH2FUZZ_CORPORA_OVERRIDE").ok();
        // source directory for sampling
        let corpora_label = target_enum.corpora();
        let ws = workspace_dir()?;
        let logs_root = self.logs_root()?;
        let source_dir: std::path::PathBuf = if let Some(label) = &arm.bin_label {
            if let Some(tmap) = self.bins.get(&arm.target_name) {
                if let Some(dir) = tmap.get(label) { std::path::PathBuf::from(dir) } else { corpora_dir()?.join(&corpora_label) }
            } else { corpora_dir()?.join(&corpora_label) }
        } else { corpora_dir()?.join(&corpora_label) };
        // prepare batch dir
        let batch_dir = logs_root.join("rl").join("rl_batch").join(&arm.target_name);
        std::fs::create_dir_all(&batch_dir)?;
        // clean previous batch files
        for e in std::fs::read_dir(&batch_dir)? { let p = e?.path(); let _ = std::fs::remove_file(p); }
        // sample up to 128 files to accelerate discoveries in short runs
        let _ = sample_batch(&source_dir, &batch_dir, 128);
        env::set_var("ETH2FUZZ_CORPORA_OVERRIDE", &batch_dir);

        // Launch run
        let start = Instant::now();
        // count hfuzz input before run
        let hfuzz_input_dir = ws.join("hfuzz").join("hfuzz_workspace").join(&arm.target_name).join("input");
        let before_units = std::fs::read_dir(&hfuzz_input_dir).map(|it| it.filter(|e| e.as_ref().ok().map(|x| x.path().is_file()).unwrap_or(false)).count()).unwrap_or(0);
        // mark mode/tag for logs placement
        std::env::set_var("ETH2FUZZ_RUN_MODE", "rl");
        std::env::set_var("ETH2FUZZ_TAG", &self.tag);
        let res = super_run_target(&arm.target_name, fuzzer, cfg);
        let dur = start.elapsed();
        let after_units = std::fs::read_dir(&hfuzz_input_dir).map(|it| it.filter(|e| e.as_ref().ok().map(|x| x.path().is_file()).unwrap_or(false)).count()).unwrap_or(before_units);
        let units_delta = after_units.saturating_sub(before_units);

        // Collect metrics for reward shaping and next-state features
        let (cur_metrics, feats12, deltas) = read_metrics_from_env(&self.last_metrics);
        self.last_metrics = cur_metrics;
        self.last_features12 = feats12.to_vec();

        // Restore env override to previous state
        match prev_override {
            Some(v) => env::set_var("ETH2FUZZ_CORPORA_OVERRIDE", v),
            None => env::remove_var("ETH2FUZZ_CORPORA_OVERRIDE"),
        }

        // Simple reward: success -> 1.0, early quit -> 0.3; bonus for full segment duration
        let base = match res {
            Ok(()) => 1.0,
            Err(_e) => 0.3,
        };
        let time_bonus = (dur.as_secs_f64() / segment.max(1) as f64).min(1.0) * 0.2;
        // Dense rewards from metrics deltas
        let mut dense = 0.0;
        if deltas.head_switch { dense += 0.05; }
        if deltas.finalized_advanced { dense += 0.2; }
        if deltas.reorg_increased { dense += 0.2; }
        // Reward for new inputs discovered in this segment (fast signal)
        let units_bonus = (units_delta as f64 * 0.01).min(0.3);

        // Prune: merge hfuzz inputs with corpora and write to run outputs
        let kept_dir = if let Some(paths) = &self.run_paths {
            let out_root = paths.outputs_root.join("corpora_pruned");
            std::fs::create_dir_all(&out_root)?;
            let kept = merge_hfuzz_and_corpora_then_prune_into(&ws, &arm.target_name, &corpora_label, 256, &out_root)?;
            Some(kept)
        } else {
            None
        };

        // Try to parse coverage percent from log
        let log_path = self.logs_root()?.join("rl").join("hfuzz").join("logs").join(format!("{}.log", arm.target_name));
        let mut cov_pct: Option<f64> = None;
        if let Ok(s) = std::fs::read_to_string(&log_path) {
            if let Some(line) = s.lines().rev().find(|l| l.contains("branch_coverage_percent")) {
                let re = Regex::new(r"branch_coverage_percent:\s*([0-9]+)").ok();
                if let Some(r) = re { if let Some(cap) = r.captures(line) { if let Some(m) = cap.get(1) { cov_pct = m.as_str().parse::<f64>().ok(); } } }
            }
        }

        // Mirror new inputs under logs tree for reproducibility (best-effort)
        let mirror_dir = self.logs_root()?.join("rl").join("hfuzz").join("input").join(&arm.target_name);
        std::fs::create_dir_all(&mirror_dir).ok();
        if let Ok(rd) = std::fs::read_dir(&hfuzz_input_dir) {
            for e in rd.flatten() {
                let p = e.path(); if p.is_file() {
                    let dst = mirror_dir.join(p.file_name().unwrap());
                    if !dst.exists() { let _ = std::fs::copy(&p, &dst); }
                }
            }
        }

        // Update run stats
        if let (Some(paths), Some(st)) = (&self.run_paths, self.run_stats.as_mut()) {
            let batch_count = std::fs::read_dir(&batch_dir).map(|it| it.filter(|e| e.as_ref().ok().map(|x| x.path().is_file()).unwrap_or(false)).count()).unwrap_or(0);
            st.segments.push(SegmentRecord { index: self.seg_index, target: arm.target_name.clone(), bin: arm.bin_label.clone(), batch_count, reward: base + time_bonus + dense + units_bonus, branch_cov_pct: cov_pct, hfuzz_units_delta: Some(units_delta) });
            if let Some(outdir) = kept_dir {
                let kept_count = std::fs::read_dir(&outdir).map(|it| it.filter(|e| e.as_ref().ok().map(|x| x.path().is_file()).unwrap_or(false)).count()).unwrap_or(0);
                // merged count = batch + prior corpus count (approx)
                let prior = std::fs::read_dir(&source_dir).map(|it| it.filter(|e| e.as_ref().ok().map(|x| x.path().is_file()).unwrap_or(false)).count()).unwrap_or(0);
                st.prunes.push(PruneRecord { target: arm.target_name.clone(), corpora_label: corpora_label.clone(), merged_count: prior + batch_count, kept_count, out_dir: outdir.to_string_lossy().to_string() });
            }
            store_stats(paths, st)?;
            self.seg_index += 1;
        }
        Ok(base + time_bonus + dense + units_bonus)
    }

    pub fn run(
        &mut self,
        filter: &str,
        fuzzer: Option<Fuzzer>,
        total_seconds: i32,
        segment_seconds: i32,
        threads: Option<i32>,
    ) -> Result<(), Error> {
        self.initialize(filter)?;
        if let (Some(paths), Some(st)) = (&self.run_paths, self.run_stats.as_mut()) {
            st.manifest.total_seconds = total_seconds;
            st.manifest.segment_seconds = segment_seconds;
            st.manifest.threads = threads;
            store_stats(paths, st)?;
        }
        let mut remaining = total_seconds.max(0);
        while remaining > 0 {
            let (idx, xs, old_probs) = if self.cfg.mode.to_lowercase().contains("ppo") {
                let (idx, xs, probs) = self.select_arm_ppo();
                (idx, xs, probs)
            } else {
                let idx = self.select_arm_ucb1();
                (idx, self.build_features_for_all_arms(), vec![1.0 / self.arms.len() as f64; self.arms.len()])
            };
            let arm = self.arms[idx].clone();
            let reward = self.run_one_segment(&arm, fuzzer, segment_seconds, threads)?;
            self.update_stat(idx, reward);

            if self.cfg.mode.to_lowercase().contains("ppo") {
                // push transition and update immediately (small batch) to avoid long waits
                self.buffer.push(Transition { xs, old_probs, chosen: idx, reward });
                let out_dir_for_policy = if self.cfg.save_policy { Some(self.logs_root()?.join("rl").join("rl_output")) } else { None };
                if let Some(pol) = self.ppo.as_mut() {
                    let _ = pol.update_from_transitions(&self.buffer);
                    if let Some(out) = out_dir_for_policy {
                        std::fs::create_dir_all(&out)?;
                        let path = if let Some(ref p) = self.cfg.policy_path { std::path::PathBuf::from(p) } else { out.join("ppo_policy.json") };
                        let v = pol.to_json();
                        let s = serde_json::to_string_pretty(&v)?;
                        let _ = std::fs::write(path, s);
                    }
                    // keep last few transitions
                    if self.buffer.len() > 8 { self.buffer.drain(0..self.buffer.len()-8); }
                }
            }
            remaining -= segment_seconds;
        }
        // Write stats
        self.persist_stats()?;
        Ok(())
    }

    fn persist_stats(&self) -> Result<(), Error> {
        let out_dir = self.logs_root()?.join("rl").join("rl_output");
        std::fs::create_dir_all(&out_dir)?;
        let mut report: Vec<serde_json::Value> = Vec::new();
        for arm in self.arms.iter() {
            let st = self.stats.get(arm).cloned().unwrap_or_default();
            report.push(serde_json::json!({
                "target": arm.target_name,
                "bin": arm.bin_label,
                "pulls": st.pulls,
                "reward_sum": st.reward_sum,
            }));
        }
        let s = serde_json::to_string_pretty(&report)?;
        std::fs::write(out_dir.join("rl_stats.json"), s)?;
        Ok(())
    }
}

fn super_run_target(
    target_name: &str,
    fuzzer: Option<Fuzzer>,
    config: FuzzerConfig,
) -> Result<(), Error> {
    // Reuse existing main.rs helper by copying minimal logic here to avoid refactor
    use Fuzzer::*;
    let target = Targets::iter()
        .find(|x| x.name() == target_name)
        .ok_or_else(|| failure::err_msg("unknown target"))?;
    let default_fuzz = match fuzzer {
        Some(o) => o,
        None => crate::fuzzers::get_default_fuzzer(target),
    };
    match default_fuzz {
        Afl => {
            let afl = crate::rust_fuzzers::FuzzerAfl::new(config)?;
            afl.run(target)?;
        }
        Honggfuzz => {
            let hfuzz = crate::rust_fuzzers::FuzzerHfuzz::new(config)?;
            hfuzz.run(target)?;
        }
        Libfuzzer => {
            let lfuzz = crate::rust_fuzzers::FuzzerLibfuzzer::new(config)?;
            lfuzz.run(target)?;
        }
        Jsfuzz => {
            let jfuzz = crate::js_fuzzers::FuzzerJsFuzz::new(config)?;
            jfuzz.run(target)?;
        }
        NimLibfuzzer => {
            let nfuzz = crate::nim_fuzzers::FuzzerNimLibfuzzer::new(config)?;
            nfuzz.run(target)?;
        }
        GoLibfuzzer => {
            let gofuzz = crate::go_fuzzers::FuzzerGoLibfuzzer::new(config)?;
            gofuzz.run(target)?;
        }
        JavaJQFAfl => {
            let javafuzz = crate::java_fuzzers::FuzzerJavaJQFAfl::new(config)?;
            javafuzz.run(target)?;
        }
    }
    Ok(())
}
// ===== Feature Engineering =====
impl RLEngine {
    fn feature_dim(&self) -> usize {
        // kinds one-hot (10) + bin one-hot (4) + pulls + mean_reward + seg_norm + threads_norm -> 10+4+4=18
        // + 12 beacon/prom features
        10 + 4 + 4 + 12
    }

    fn build_features_for_all_arms(&self) -> Vec<Vec<f64>> {
        let mut xs = Vec::with_capacity(self.arms.len());
        for (i, arm) in self.arms.iter().enumerate() {
            xs.push(self.build_features_for_arm(i, arm));
        }
        xs
    }

    fn build_features_for_arm(&self, idx: usize, arm: &ArmKey) -> Vec<f64> {
        let mut v = vec![0.0; self.feature_dim()];
        let mut off = 0usize;
        // kind one-hot
        let kind = self.corpora_kind_index(&arm.target_name);
        if kind < 10 { v[off + kind] = 1.0; }
        off += 10;
        // bin one-hot: small, medium, large, none
        let bin_idx = match arm.bin_label.as_deref() { Some("small") => 0, Some("medium") => 1, Some("large") => 2, _ => 3 };
        v[off + bin_idx] = 1.0; off += 4;
        // pulls (log scaled), mean reward, seg_norm, threads_norm
        let st = self.stats.get(arm).cloned().unwrap_or_default();
        let pulls = (st.pulls as f64 + 1.0).ln() / 5.0; v[off] = pulls; off += 1;
        let mean = if st.pulls > 0 { st.reward_sum / st.pulls as f64 } else { 0.0 }; v[off] = mean; off += 1;
        // default norms
        v[off] = 1.0; off += 1; // seg_norm placeholder (not wired)
        v[off] = 1.0; off += 1; // threads_norm placeholder (not wired)
        // append 12-d metrics features (global context)
        for val in self.last_features12.iter() { v[off] = *val; off += 1; }
        v
    }

    fn corpora_kind_index(&self, target_name: &str) -> usize {
        // Map by corpora label inferred from target enum
        let t = Targets::iter().find(|x| x.name() == target_name).unwrap();
        let k = t.corpora();
        match k.as_str() {
            "attestation" => 0,
            "attester_slashing" => 1,
            "block" => 2,
            "block_header" => 3,
            "deposit" => 4,
            "proposer_slashing" => 5,
            "voluntary_exit" => 6,
            "enr" => 7,
            "bls" => 8,
            "discv5_packet" => 9,
            _ => 9,
        }
    }

    fn logs_root(&self) -> Result<std::path::PathBuf, Error> {
        Ok(workspace_dir()?.join("logs").join(&self.tag))
    }

    fn context_bias_for_arm(&self, arm: &ArmKey) -> f64 {
        // Heuristic bias: on epoch boundary or head switch/reorg, prefer attestation & block_header
        let kind_idx = self.corpora_kind_index(&arm.target_name);
        // decode some context from last features
        let slot_mod = self.last_features12.get(0).cloned().unwrap_or(0.0);
        let head_switch = self.last_features12.get(10).cloned().unwrap_or(0.0) > 0.5;
        let reorg_bucket = self.last_features12.get(2).cloned().unwrap_or(0.0);
        let near_boundary = slot_mod <= 0.05 || slot_mod >= 0.95;
        let is_attestation = kind_idx == 0;
        let is_block_header = kind_idx == 3;
        let mut w = 1.0;
        if near_boundary || head_switch || reorg_bucket > 0.5 {
            if is_attestation || is_block_header { w *= 1.2; }
        }
        w
    }
}
