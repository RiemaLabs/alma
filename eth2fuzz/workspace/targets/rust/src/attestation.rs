use state_processing::{per_block_processing::verify_attestation_for_state, ConsensusContext};

use state_processing::per_block_processing::VerifySignatures;
use types::{Attestation, AttestationRef, BeaconState, ChainSpec, EthSpec, MainnetEthSpec};

/// Run `process_block_header`
pub fn process_attestation(
    beaconstate: BeaconState<MainnetEthSpec>,
    attestation: Attestation<MainnetEthSpec>,
) -> Result<(), ()> {
    let spec: ChainSpec = MainnetEthSpec::default_spec();
    let mut ctxt = ConsensusContext::new(beaconstate.slot());
    // Borrow as AttestationRef
    let att_ref: AttestationRef<MainnetEthSpec> = match &attestation {
        Attestation::Base(inner) => AttestationRef::Base(inner),
        Attestation::Electra(inner) => AttestationRef::Electra(inner),
    };

    // Validate attestation against the given state.
    let _ = verify_attestation_for_state(
        &beaconstate,
        att_ref,
        &mut ctxt,
        VerifySignatures::True,
        &spec,
    );

    Ok(())
}
