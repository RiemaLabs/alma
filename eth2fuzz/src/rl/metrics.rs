use failure::Error;
use serde::Deserialize;

#[derive(Default, Clone, Debug)]
pub struct BeaconSnapshot {
    pub head_slot: Option<u64>,
    pub head_root: Option<String>,
    pub justified_epoch: Option<u64>,
    pub finalized_epoch: Option<u64>,
}

#[derive(Default, Clone, Debug)]
pub struct PromSnapshot {
    pub reorg_count: Option<u64>,
    pub participation_rate: Option<f64>,
    pub inactivity_p99: Option<f64>,
    pub pending_slashings: Option<u64>,
    pub is_syncing: Option<bool>,
}

#[derive(Default, Clone, Debug)]
pub struct MetricsCtx {
    pub beacon: BeaconSnapshot,
    pub prom: PromSnapshot,
}

#[derive(Default, Clone, Debug)]
pub struct SegmentDelta {
    pub head_switch: bool,
    pub finalized_advanced: bool,
    pub reorg_increased: bool,
}

#[derive(Default, Clone, Debug)]
pub struct RLFeatures12 {
    // 12 basic features summarized as scalars in [0,1] if possible
    pub slot_mod_epoch_norm: f64,
    pub head_slot_norm: f64,
    pub reorg_depth_bucket: f64,
    pub justified_epoch_norm: f64,
    pub finalized_epoch_norm: f64,
    pub participation_rate_bucket: f64,
    pub inactivity_score_p99_bucket: f64,
    pub num_pending_slashings_bucket: f64,
    pub unrealized_finalized_flag: f64,
    pub client_sync_state_flag: f64,
    pub head_switch_flag: f64,
    pub finalized_adv_flag: f64,
}

impl RLFeatures12 {
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.slot_mod_epoch_norm,
            self.head_slot_norm,
            self.reorg_depth_bucket,
            self.justified_epoch_norm,
            self.finalized_epoch_norm,
            self.participation_rate_bucket,
            self.inactivity_score_p99_bucket,
            self.num_pending_slashings_bucket,
            self.unrealized_finalized_flag,
            self.client_sync_state_flag,
            self.head_switch_flag,
            self.finalized_adv_flag,
        ]
    }
}

#[derive(Deserialize)]
struct HeaderData {
    slot: String,
    root: String,
}
#[derive(Deserialize)]
struct HeaderResponse {
    data: HeaderData,
}
#[derive(Deserialize)]
struct Ckpt {
    epoch: String,
}
#[derive(Deserialize)]
struct FinalityData {
    previous_justified: Ckpt,
    current_justified: Ckpt,
    finalized: Ckpt,
}
#[derive(Deserialize)]
struct FinalityResponse {
    data: FinalityData,
}

pub fn collect_beacon(api: &str) -> Result<BeaconSnapshot, Error> {
    // GET /eth/v1/beacon/headers/head
    let head: HeaderResponse = ureq::get(&format!("{}/eth/v1/beacon/headers/head", api))
        .call()?
        .into_json()?;
    let slot = head.data.slot.parse::<u64>().ok();
    let root = Some(head.data.root);
    // GET /eth/v1/beacon/states/head/finality_checkpoints
    let fin: FinalityResponse = ureq::get(&format!(
        "{}/eth/v1/beacon/states/head/finality_checkpoints",
        api
    ))
    .call()?
    .into_json()?;
    let justified = fin.data.current_justified.epoch.parse::<u64>().ok();
    let finalized = fin.data.finalized.epoch.parse::<u64>().ok();
    Ok(BeaconSnapshot {
        head_slot: slot,
        head_root: root,
        justified_epoch: justified,
        finalized_epoch: finalized,
    })
}

pub fn collect_prom(endpoint: &str) -> Result<PromSnapshot, Error> {
    // scrape /metrics and look for common reorg counters (best-effort)
    let resp = ureq::get(
        &(if endpoint.ends_with("/metrics") {
            endpoint.to_string()
        } else {
            format!("{}/metrics", endpoint)
        }),
    )
    .call()?
    .into_string()?;
    let mut reorg_count: Option<u64> = None;
    let mut participation_rate: Option<f64> = None;
    let mut inactivity_p99: Option<f64> = None;
    let mut pending_slashings: Option<u64> = None;
    let mut is_syncing: Option<bool> = None;
    for line in resp.lines() {
        if line.starts_with('#') {
            continue;
        }
        if line.contains("reorg_count") || line.contains("fork_choice_reorg") {
            // naive parse: last token as integer
            if let Some(tok) = line.split_whitespace().last() {
                if let Ok(v) = tok.parse::<u64>() {
                    reorg_count = Some(v);
                    break;
                }
            }
        }
    }
    // second pass for other metrics
    for line in resp.lines() {
        if line.starts_with('#') {
            continue;
        }
        let lower = line.to_lowercase();
        // participation rate as ratio 0..1
        if participation_rate.is_none()
            && lower.contains("participation")
            && !lower.contains("help")
        {
            if let Some(tok) = line.split_whitespace().last() {
                if let Ok(mut v) = tok.parse::<f64>() {
                    if v > 1.0 {
                        v = (v / 100.0).min(1.0);
                    }
                    if v >= 0.0 && v <= 1.0 {
                        participation_rate = Some(v);
                    }
                }
            }
        }
        // inactivity score p99
        if inactivity_p99.is_none() && lower.contains("inactivity") && lower.contains("p99") {
            if let Some(tok) = line.split_whitespace().last() {
                if let Ok(v) = tok.parse::<f64>() {
                    inactivity_p99 = Some(v);
                }
            }
        }
        // pending slashings
        if pending_slashings.is_none()
            && (lower.contains("pending_slashings") || lower.contains("attester_slashings"))
        {
            if let Some(tok) = line.split_whitespace().last() {
                if let Ok(v) = tok.parse::<u64>() {
                    pending_slashings = Some(v);
                }
            }
        }
        // sync state
        if is_syncing.is_none()
            && lower.contains("sync")
            && (lower.contains("is_syncing") || lower.ends_with("syncing"))
        {
            if let Some(tok) = line.split_whitespace().last() {
                if let Ok(v) = tok.parse::<u64>() {
                    is_syncing = Some(v != 0);
                }
            }
        }
    }
    Ok(PromSnapshot {
        reorg_count,
        participation_rate,
        inactivity_p99,
        pending_slashings,
        is_syncing,
    })
}

pub fn compute_features(prev: &MetricsCtx, cur: &MetricsCtx) -> RLFeatures12 {
    let mut f = RLFeatures12::default();
    if let Some(slot) = cur.beacon.head_slot {
        f.slot_mod_epoch_norm = ((slot % 32) as f64) / 31.0;
        f.head_slot_norm = (slot as f64).log10() / 6.0;
    }
    if let Some(j) = cur.beacon.justified_epoch {
        f.justified_epoch_norm = (j as f64).log10() / 6.0;
    }
    if let Some(fin) = cur.beacon.finalized_epoch {
        f.finalized_epoch_norm = (fin as f64).log10() / 6.0;
    }
    // reorg bucket — diff of prom counter
    let mut reorg_delta = 0u64;
    if let (Some(a), Some(b)) = (prev.prom.reorg_count, cur.prom.reorg_count) {
        if b > a {
            reorg_delta = b - a;
        }
    }
    f.reorg_depth_bucket = if reorg_delta == 0 {
        0.0
    } else if reorg_delta <= 2 {
        0.5
    } else {
        1.0
    };
    // participation bucket
    if let Some(p) = cur.prom.participation_rate {
        f.participation_rate_bucket = (p.max(0.0).min(1.0) * 1.0) as f64;
    }
    // inactivity p99 bucket (heuristic scale)
    if let Some(ix) = cur.prom.inactivity_p99 {
        f.inactivity_score_p99_bucket = ((ix / 64.0).min(1.0)) as f64;
    }
    // pending slashings bucket: log scale normalize
    if let Some(ps) = cur.prom.pending_slashings {
        f.num_pending_slashings_bucket = ((ps as f64 + 1.0).ln() / 6.0).min(1.0);
    }
    // unrealized finalized flag: justified - finalized >= 1
    if let (Some(j), Some(fin)) = (cur.beacon.justified_epoch, cur.beacon.finalized_epoch) {
        if j > fin {
            f.unrealized_finalized_flag = 1.0;
        }
    }
    // sync state flag
    if let Some(true) = cur.prom.is_syncing {
        f.client_sync_state_flag = 1.0;
    }
    // head switch flag
    if let (Some(pr), Some(cr)) = (&prev.beacon.head_root, &cur.beacon.head_root) {
        if pr != cr {
            f.head_switch_flag = 1.0;
        }
    }
    // finalized advanced flag
    if let (Some(pe), Some(ce)) = (prev.beacon.finalized_epoch, cur.beacon.finalized_epoch) {
        if ce > pe {
            f.finalized_adv_flag = 1.0;
        }
    }
    f
}

pub fn delta_events(prev: &MetricsCtx, cur: &MetricsCtx) -> SegmentDelta {
    let mut d = SegmentDelta::default();
    if let (Some(pr), Some(cr)) = (&prev.beacon.head_root, &cur.beacon.head_root) {
        if pr != cr {
            d.head_switch = true;
        }
    }
    if let (Some(pe), Some(ce)) = (prev.beacon.finalized_epoch, cur.beacon.finalized_epoch) {
        if ce > pe {
            d.finalized_advanced = true;
        }
    }
    if let (Some(a), Some(b)) = (prev.prom.reorg_count, cur.prom.reorg_count) {
        if b > a {
            d.reorg_increased = true;
        }
    }
    d
}

pub fn read_metrics_from_env(prev: &MetricsCtx) -> (MetricsCtx, RLFeatures12, SegmentDelta) {
    let mut cur = MetricsCtx::default();
    if let Ok(api) = std::env::var("ETH2FUZZ_BEACON_API") {
        if !api.is_empty() {
            if let Ok(b) = collect_beacon(&api) {
                cur.beacon = b;
            }
        }
    }
    if let Ok(prom) = std::env::var("ETH2FUZZ_PROM_ENDPOINT") {
        if !prom.is_empty() {
            if let Ok(p) = collect_prom(&prom) {
                cur.prom = p;
            }
        }
    }
    let feats = compute_features(prev, &cur);
    let delta = delta_events(prev, &cur);
    (cur, feats, delta)
}
