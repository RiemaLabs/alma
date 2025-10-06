extern crate ssz;

use ssz::Decode;
use types::{
    attestation::AttestationOnDisk, attester_slashing::AttesterSlashingOnDisk, Attestation,
    AttesterSlashing, BeaconBlock, BeaconState, ChainSpec, Deposit, EthSpec, MainnetEthSpec,
    ProposerSlashing, SignedBeaconBlock, SignedVoluntaryExit,
};

mod block;
#[inline(always)]
pub fn fuzz_lighthouse_block(beaconstate: BeaconState<MainnetEthSpec>, data: &[u8]) {
    // Decode a SignedBeaconBlock with spec context and try a per-block state transition.
    let spec: ChainSpec = MainnetEthSpec::default_spec();
    let block = match SignedBeaconBlock::from_ssz_bytes(&data, &spec) {
        Ok(block) => block,
        Err(_e) => return,
    };
    let _ = block::state_transition(beaconstate, block, true);
}

mod block_header;
#[inline(always)]
pub fn fuzz_lighthouse_block_header(beaconstate: BeaconState<MainnetEthSpec>, data: &[u8]) {
    let spec: ChainSpec = MainnetEthSpec::default_spec();
    let block = match BeaconBlock::from_ssz_bytes(&data, &spec) {
        Ok(block) => block,
        Err(_e) => return,
    };
    let _ = block_header::process_header(beaconstate, block);
}

mod attestation;
#[inline(always)]
pub fn fuzz_lighthouse_attestation(beaconstate: BeaconState<MainnetEthSpec>, data: &[u8]) {
    let attestation: Attestation<MainnetEthSpec> = match AttestationOnDisk::from_ssz_bytes(&data) {
        Ok(a) => a.into(),
        Err(_) => return,
    };
    let _ = attestation::process_attestation(beaconstate, attestation);
}

mod attester_slashing;
#[inline(always)]
pub fn fuzz_lighthouse_attester_slashing(
    beaconstate: BeaconState<MainnetEthSpec>,
    data: &[u8],
) {
    let slashing: AttesterSlashing<MainnetEthSpec> = match AttesterSlashingOnDisk::from_ssz_bytes(&data) {
        Ok(s) => s.into(),
        Err(_) => return,
    };
    let _ = attester_slashing::process_attester_slashing(beaconstate, slashing);
}

mod proposer_slashing;
#[inline(always)]
pub fn fuzz_lighthouse_proposer_slashing(
    beaconstate: BeaconState<MainnetEthSpec>,
    data: &[u8],
) {
    let s: ProposerSlashing = match ProposerSlashing::from_ssz_bytes(&data) {
        Ok(s) => s,
        Err(_) => return,
    };
    let _ = proposer_slashing::process_proposer_slashing(beaconstate, s);
}

mod deposit;
#[inline(always)]
pub fn fuzz_lighthouse_deposit(beaconstate: BeaconState<MainnetEthSpec>, data: &[u8]) {
    let d: Deposit = match Deposit::from_ssz_bytes(&data) {
        Ok(d) => d,
        Err(_) => return,
    };
    let _ = deposit::process_deposit(beaconstate, d);
}

mod voluntary_exit;
#[inline(always)]
pub fn fuzz_lighthouse_voluntary_exit(beaconstate: BeaconState<MainnetEthSpec>, data: &[u8]) {
    let e: SignedVoluntaryExit = match SignedVoluntaryExit::from_ssz_bytes(&data) {
        Ok(e) => e,
        Err(_) => return,
    };
    let _ = voluntary_exit::process_voluntary_exit(beaconstate, e);
}

/* libp2p */

#[inline(always)]
pub fn fuzz_lighthouse_enr(data: &[u8]) {
    // TODO - could be improved
    // will be better to craft "enr:" + base64encode(data)
    use lighthouse_network::Enr;
    use std::str;
    use std::str::FromStr;
    // data will be convert into str first
    let d = match str::from_utf8(&data) {
        Ok(d) => d,
        _ => return,
    };
    let _a = Enr::from_str(&d);
}

/* BLS */

#[inline(always)]
pub fn fuzz_lighthouse_bls(data: &[u8]) {
    use bls::Signature;
    let _ = Signature::deserialize(&data);
}

/* discv5 */
pub fn fuzz_lighthouse_discv5_packet(data: &[u8]) {
    use lighthouse_network::discv5::{enr::NodeId, packet::{Packet, ProtocolIdentity}};

    if data.len() > 32 {
        let dst_id = NodeId::parse(&data[..32]).unwrap_or_else(|_| NodeId::parse(&[0u8; 32]).unwrap());
        let bytes = &data[32..];
        let _ = Packet::decode(&dst_id, ProtocolIdentity::default(), bytes)
            .map(|(p, _)| p.encode(&dst_id));
    }
}

// BeaconState accessor fuzz (minimal)
#[inline(always)]
pub fn fuzz_lighthouse_beaconstate(data: &[u8]) {
    let spec: ChainSpec = MainnetEthSpec::default_spec();
    let mut state: BeaconState<MainnetEthSpec> = match BeaconState::from_ssz_bytes(&data, &spec) {
        Ok(s) => s,
        Err(_) => return,
    };
    // Touch a few safe methods
    let _ = state.update_tree_hash_cache();
    let _ = state.current_epoch();
}
