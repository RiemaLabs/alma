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
use types::{BeaconState, ChainSpec, EthSpec, MainnetEthSpec, BeaconBlock};
extern crate state_processing;
use state_processing::per_block_processing::process_block_header;
use state_processing::{ConsensusContext, VerifyBlockRoot};

// Grandine direct-call
extern crate gtypes;
extern crate gssz;
extern crate gtrans;
use gssz::SszRead as GSszRead;
use gtrans::unphased as gunphased;
use gtypes::{combined as gcombined, config::Config as GConfig, preset::Mainnet as GMainnet};

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};

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
    if let Ok(mut f) = OpenOptions::new().append(true).create(true).open("diff_fuzz_log.txt") { let _ = writeln!(f, "{}", line); }
}

struct BeaconCtx { state: BeaconState<MainnetEthSpec>, path: String }

lazy_static! {
    static ref CTX: BeaconCtx = {
        use std::env; let beacon_dir = env::var("ETH2FUZZ_BEACONSTATE").unwrap_or_default(); if beacon_dir.is_empty(){panic!("ETH2FUZZ_BEACONSTATE not set");}
        let mut list = list_files_in_folder(&beacon_dir).expect("list beaconstate dir"); use rand::seq::SliceRandom; use rand::thread_rng; list.shuffle(&mut thread_rng());
        let mut chosen = String::new(); let mut loaded: Option<BeaconState<MainnetEthSpec>> = None; for p in list.into_iter(){ if let Ok(s)=read_beaconstate(&p){ chosen=p; loaded=Some(s); break; } }
        let state = loaded.expect("at least one valid beaconstate"); append_log(&format!("beaconstate: {}", chosen)); BeaconCtx{state, path: chosen}
    };
}

fn lighthouse_verdict(state: &BeaconState<MainnetEthSpec>, block: &BeaconBlock<MainnetEthSpec>) -> Option<bool> {
    if state.slot() != block.slot() { return None; }
    let spec: ChainSpec = MainnetEthSpec::default_spec();
    let mut s = state.clone();
    let mut ctxt = ConsensusContext::new(s.slot());
    let res = process_block_header(&mut s, block.temporary_block_header(), VerifyBlockRoot::True, &mut ctxt, &spec);
    Some(res.is_ok())
}

// Grandine
lazy_static!{ static ref GCONFIG: GConfig = GConfig::mainnet(); static ref GPUBKEY: gpubkey_cache::PubkeyCache = gpubkey_cache::PubkeyCache::default(); }
fn grandine_state_from_path(path: &str) -> Option<gcombined::BeaconState<GMainnet>> {
    let bytes = std::fs::read(path).ok()?; gcombined::BeaconState::<GMainnet>::from_ssz(&*GCONFIG, &bytes).ok()
}
fn grandine_verdict(state_path: &str, block_bytes: &[u8]) -> Option<bool> {
    let mut gstate = grandine_state_from_path(state_path)?;
    // Reuse block trusted transition if input is a SignedBeaconBlock; otherwise skip
    if let Ok(signed) = gtypes::combined::SignedBeaconBlock::<GMainnet>::from_ssz(&*GCONFIG, block_bytes) {
        return Some(gtrans::combined::trusted_state_transition(&*GCONFIG, &*GPUBKEY, &mut gstate, &signed).is_ok());
    }
    None
}

fuzz_target!(|data: &[u8]| {
    let spec: ChainSpec = MainnetEthSpec::default_spec();
    let block = match BeaconBlock::from_ssz_bytes(data, &spec) { Ok(b) => b, Err(_) => return };
    if let Some(lh_ok) = lighthouse_verdict(&CTX.state, &block) {
        if let Some(gr_ok) = grandine_verdict(&CTX.path, data) { if lh_ok != gr_ok { panic!("DIFF MISMATCH: lighthouse={}, grandine={}", lh_ok, gr_ok); } }
    }
});
