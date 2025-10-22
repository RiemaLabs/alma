#![no_main]
extern crate libfuzzer_sys;
use libfuzzer_sys::fuzz_target;

extern crate lazy_static;
use lazy_static::lazy_static;

extern crate walkdir;
use walkdir::WalkDir;

extern crate ssz;
use ssz::Decode;

extern crate types;
use types::{
    attestation::AttestationOnDisk, Attestation, AttestationRef, BeaconState, ChainSpec, EthSpec,
    MainnetEthSpec,
};

// Lighthouse validation (existing)
extern crate state_processing;
use state_processing::{per_block_processing::verify_attestation_for_state, ConsensusContext};
use state_processing::per_block_processing::VerifySignatures;

// Grandine direct-call
extern crate gtypes;
extern crate gssz;
extern crate gtrans;
extern crate gpubkey_cache;
extern crate ghelper;
use gpubkey_cache::PubkeyCache as GPubkeyCache;
use gssz::SszRead as GSszRead;
use gtrans::unphased as gunphased;
use ghelper::verifier::NullVerifier as GNullVerifier;
use gtypes::{combined as gcombined, config::Config as GConfig, preset::Mainnet as GMainnet};

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

extern crate rand;
use rand::seq::SliceRandom;
use rand::thread_rng;

#[inline(always)]
fn list_files_in_folder(path_str: &str) -> Result<Vec<String>, ()> {
    let mut list: Vec<String> = Vec::new();
    for entry in WalkDir::new(path_str).into_iter().filter_map(|e| e.ok()) {
        if entry.metadata().map(|m| m.is_file()).unwrap_or(false) {
            list.push(entry.path().display().to_string());
        }
    }
    Ok(list)
}

#[inline(always)]
fn read_file(path: &str) -> io::Result<Vec<u8>> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut file = File::open(path)?;
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}

#[inline(always)]
fn read_beaconstate(path: &str) -> Result<BeaconState<MainnetEthSpec>, ssz::DecodeError> {
    let bytes = read_file(path).map_err(|_| ssz::DecodeError::BytesInvalid("io".into()))?;
    BeaconState::from_ssz_bytes(&bytes, &MainnetEthSpec::default_spec())
}

fn append_log(line: &str) {
    if let Ok(mut f) = OpenOptions::new().append(true).create(true).open("diff_fuzz_log.txt") {
        let _ = writeln!(f, "{}", line);
    }
}

struct BeaconCtx {
    state: BeaconState<MainnetEthSpec>,
    path: String,
}

lazy_static! {
    static ref CTX: BeaconCtx = {
        use std::env;
        let key = "ETH2FUZZ_BEACONSTATE";
        let beacon_dir = env::var(key).unwrap_or_default();
        if beacon_dir.is_empty() {
            panic!("ETH2FUZZ_BEACONSTATE not set");
        }
        let mut list = list_files_in_folder(&beacon_dir).expect("list beaconstate dir");
        list.shuffle(&mut thread_rng());

        let mut chosen = String::new();
        let mut loaded: Option<BeaconState<MainnetEthSpec>> = None;
        for p in list.into_iter() {
            match read_beaconstate(&p) {
                Ok(s) => { chosen = p; loaded = Some(s); break; }
                Err(_) => continue,
            }
        }
        let state = loaded.expect("at least one valid beaconstate");
        append_log(&format!("beaconstate: {}", chosen));
        BeaconCtx { state, path: chosen }
    };
}

fn lighthouse_verdict_details(
    state: &BeaconState<MainnetEthSpec>,
    att: &Attestation<MainnetEthSpec>,
) -> (bool, Option<String>) {
    let mut ctxt = ConsensusContext::new(state.slot());
    let att_ref: AttestationRef<MainnetEthSpec> = match att {
        Attestation::Base(inner) => AttestationRef::Base(inner),
        Attestation::Electra(inner) => AttestationRef::Electra(inner),
    };
    let res = verify_attestation_for_state(
        state,
        att_ref,
        &mut ctxt,
        VerifySignatures::False,
        &MainnetEthSpec::default_spec(),
    );
    match res {
        Ok(_) => (true, None),
        Err(e) => (false, Some(format!("{:?}", e))),
    }
}

// Grandine shared context
lazy_static! {
    static ref GCONFIG: GConfig = GConfig::mainnet();
    static ref GPUBKEY: GPubkeyCache = GPubkeyCache::default();
}

fn grandine_state_from_path(path: &str) -> Option<gcombined::BeaconState<GMainnet>> {
    let bytes = std::fs::read(path).ok()?;
    gcombined::BeaconState::<GMainnet>::from_ssz(&*GCONFIG, &bytes).ok()
}

fn grandine_verdict_attestation(state_path: &str, att_bytes: &[u8]) -> Option<(bool, Option<String>)> {
    let gstate = grandine_state_from_path(state_path)?;
    // Try Electra then Phase0
    if let Ok(gatt_e) = gtypes::electra::containers::Attestation::<GMainnet>::from_ssz(&*GCONFIG, att_bytes) {
        // Disable signature checks for fuzzing on Grandine side as well
        let res = gtrans::electra::validate_attestation_with_verifier(&*GCONFIG, &*GPUBKEY, &gstate, &gatt_e, GNullVerifier);
        return Some(match res {
            Ok(()) => (true, None),
            Err(e) => (false, Some(format!("{}", e))),
        });
    }
    if let Ok(_gatt_p) = gtypes::phase0::containers::Attestation::<GMainnet>::from_ssz(&*GCONFIG, att_bytes) {
        // Phase0 path in Grandine uses SingleVerifier internally; to keep signatures off, skip
        return None;
    }
    None
}

fuzz_target!(|data: &[u8]| {
    // Decode attestation
    let att: Attestation<MainnetEthSpec> = match AttestationOnDisk::from_ssz_bytes(data) {
        Ok(a) => a.into(),
        Err(_) => return,
    };

    // Gating to avoid comparing apples to oranges (phase/window/target invariants):
    // - phase_at_slot(state.slot) == phase_at_slot(att.slot)
    // - state.slot in [att.slot + MIN_DELAY, att.slot + SLOTS_PER_EPOCH]
    // - att.target.epoch == epoch(att.slot)
    // - LH variant (Base/Electra) matches phase_at_slot(att.slot)
    {
        let spec: ChainSpec = MainnetEthSpec::default_spec();
        let slots_per_epoch = MainnetEthSpec::slots_per_epoch();

        // Extract att.slot and att.target.epoch
        let (att_slot, att_target_epoch, att_is_electra_variant) = match &att {
            Attestation::Base(inner) => (inner.data.slot, inner.data.target.epoch, false),
            Attestation::Electra(inner) => (inner.data.slot, inner.data.target.epoch, true),
        };

        let state_slot = CTX.state.slot();
        let att_epoch = att_slot.epoch(slots_per_epoch);
        let state_epoch = state_slot.epoch(slots_per_epoch);

        // Electra fork gating via epoch
        let electra_fork_epoch = spec.electra_fork_epoch;
        let state_is_electra = electra_fork_epoch
            .map(|e| state_epoch >= e)
            .unwrap_or(false);
        let att_is_electra = electra_fork_epoch
            .map(|e| att_epoch >= e)
            .unwrap_or(false);

        // phase_at_slot(state.slot) == phase_at_slot(att.slot)
        if state_is_electra != att_is_electra {
            return;
        }
        // LH variant matches expected phase for att.slot
        if att_is_electra_variant != att_is_electra {
            return;
        }
        // inclusion window: state.slot ∈ [att.slot + MIN_DELAY, att.slot + SLOTS_PER_EPOCH]
        let state_u64 = state_slot.as_u64();
        let att_u64 = att_slot.as_u64();
        let low = att_u64.saturating_add(spec.min_attestation_inclusion_delay);
        let high = att_u64.saturating_add(slots_per_epoch);
        if state_u64 < low || state_u64 > high { return; }
        // target epoch matches
        if att_target_epoch != att_epoch { return; }
    }

    // Verdicts with diagnostics
    let (lh_ok, lh_err) = lighthouse_verdict_details(&CTX.state, &att);
    if let Some((gr_ok, gr_err)) = grandine_verdict_attestation(&CTX.path, data) {
        if lh_ok != gr_ok {
            // Collect context for debugging
            let spec: ChainSpec = MainnetEthSpec::default_spec();
            let slots_per_epoch = MainnetEthSpec::slots_per_epoch();
            let (att_slot, att_target_epoch, variant_electra) = match &att {
                Attestation::Base(inner) => (inner.data.slot, inner.data.target.epoch, false),
                Attestation::Electra(inner) => (inner.data.slot, inner.data.target.epoch, true),
            };
            let state_slot = CTX.state.slot();
            let att_epoch = att_slot.epoch(slots_per_epoch);
            let state_epoch = state_slot.epoch(slots_per_epoch);
            let electra_fork_epoch = spec.electra_fork_epoch;
            let state_is_electra = electra_fork_epoch.map(|e| state_epoch >= e).unwrap_or(false);
            let att_is_electra = electra_fork_epoch.map(|e| att_epoch >= e).unwrap_or(false);
            append_log(&format!(
                "[diff-attestation] state_slot={} att_slot={} att_epoch={} target_epoch={} state_is_electra={} att_is_electra={} lh_ok={} gr_ok={} lh_err={:?} gr_err={:?} variant_electra={}",
                state_slot.as_u64(),
                att_slot.as_u64(),
                att_epoch.as_u64(),
                att_target_epoch.as_u64(),
                state_is_electra, att_is_electra,
                lh_ok, gr_ok,
                lh_err, gr_err,
                variant_electra
            ));
            panic!("DIFF MISMATCH: lighthouse={}, grandine={}", lh_ok, gr_ok);
        }
    }
});
