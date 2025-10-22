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
use types::{BeaconState, ChainSpec, EthSpec, MainnetEthSpec, ProposerSlashing, RelativeEpoch};
extern crate state_processing;
use state_processing::per_block_processing::process_operations::process_proposer_slashings;
use state_processing::{ConsensusContext, VerifySignatures};

// Grandine direct-call
extern crate gtypes; extern crate gssz; extern crate gtrans; extern crate gpubkey_cache;
use gssz::SszRead as GSszRead; use gpubkey_cache::PubkeyCache as GPubkeyCache; use gtrans::unphased as gunphased;
use gtypes::{config::Config as GConfig, preset::Mainnet as GMainnet};

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};

#[inline(always)] fn list_files_in_folder(p:&str)->Result<Vec<String>,()>{let mut v=Vec::new();for e in WalkDir::new(p).into_iter().filter_map(|e|e.ok()){if e.metadata().map(|m|m.is_file()).unwrap_or(false){v.push(e.path().display().to_string());}}Ok(v)}
#[inline(always)] fn read_file(p:&str)->io::Result<Vec<u8>>{let mut b=Vec::new();let mut f=File::open(p)?;use std::io::Read;f.read_to_end(&mut b)?;Ok(b)}
#[inline(always)] fn read_beaconstate(p:&str)->Result<BeaconState<MainnetEthSpec>,ssz::DecodeError>{let bytes=read_file(p).map_err(|_|ssz::DecodeError::BytesInvalid("io".into()))?;BeaconState::from_ssz_bytes(&bytes,&MainnetEthSpec::default_spec())}
fn append_log(line:&str){if let Ok(mut f)=OpenOptions::new().append(true).create(true).open("diff_fuzz_log.txt"){let _=writeln!(f,"{}",line);}}

struct BeaconCtx{state:BeaconState<MainnetEthSpec>,path:String}
lazy_static!{static ref CTX:BeaconCtx={use std::env;let d=env::var("ETH2FUZZ_BEACONSTATE").unwrap_or_default();if d.is_empty(){panic!("ETH2FUZZ_BEACONSTATE not set");}let mut l=list_files_in_folder(&d).expect("list");use rand::seq::SliceRandom;use rand::thread_rng;l.shuffle(&mut thread_rng());let mut c=String::new();let mut s=None;for p in l.into_iter(){if let Ok(st)=read_beaconstate(&p){c=p;s=Some(st);break;}}let state=s.expect("valid state");append_log(&format!("beaconstate: {}",c));BeaconCtx{state,path:c}};}

fn lighthouse_verdict(state:&BeaconState<MainnetEthSpec>, x:&ProposerSlashing)->bool{let spec:ChainSpec=MainnetEthSpec::default_spec();let mut s=state.clone();if s.build_committee_cache(RelativeEpoch::Current,&spec).is_err(){return false;}let mut ctxt=ConsensusContext::new(s.slot());process_proposer_slashings(&mut s,&[x.clone()],VerifySignatures::False,&mut ctxt,&spec).is_ok()}

lazy_static!{ static ref GCONFIG:GConfig=GConfig::mainnet(); static ref GPUBKEY:GPubkeyCache=GPubkeyCache::default(); }
// Grandine compare using with_verifier + NullVerifier (签名关闭)
extern crate ghelper; use ghelper::verifier::NullVerifier as GNullVerifier;
fn grandine_verdict(bytes:&[u8], state_path:&str)->Option<bool>{
    let x = gtypes::phase0::containers::ProposerSlashing::from_ssz(&*GCONFIG, bytes).ok()?;
    let st = std::fs::read(state_path).ok()?; let gstate = gtypes::combined::BeaconState::<GMainnet>::from_ssz(&*GCONFIG,&st).ok()?;
    Some(transition_functions::unphased::block_processing::validate_proposer_slashing_with_verifier(&*GCONFIG,&*GPUBKEY,&gstate,x,GNullVerifier).is_ok())
}

fuzz_target!(|data:&[u8]|{let x:ProposerSlashing=match ProposerSlashing::from_ssz_bytes(data){Ok(v)=>v,Err(_)=>return};let lh_ok=lighthouse_verdict(&CTX.state,&x); if let Some(gr_ok)=grandine_verdict(data,&CTX.path){ if lh_ok!=gr_ok{panic!("DIFF MISMATCH: lighthouse={}, grandine={}",lh_ok,gr_ok);} }});
