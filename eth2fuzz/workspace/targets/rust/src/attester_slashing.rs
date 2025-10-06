use state_processing::{
    per_block_processing::process_operations::process_attester_slashings,
    ConsensusContext, BlockProcessingError, VerifySignatures,
};

use types::{AttesterSlashing, AttesterSlashingRef, BeaconState, EthSpec, MainnetEthSpec, RelativeEpoch};

/// Run `process_attester_slashings`
pub fn process_attester_slashing(
    mut beaconstate: BeaconState<MainnetEthSpec>,
    attester_slashing: AttesterSlashing<MainnetEthSpec>,
) -> Result<(), BlockProcessingError> {
    let spec = MainnetEthSpec::default_spec();

    let state = &mut beaconstate;
    // Ensure the current epoch cache is built.
    // Required by slash_validator->initiate_validator_exit->get_churn_limit
    state.build_committee_cache(RelativeEpoch::Current, &spec)?;

    let mut ctxt = ConsensusContext::new(state.slot());
    let iter = std::iter::once(match &attester_slashing {
        AttesterSlashing::Base(inner) => AttesterSlashingRef::Base(inner),
        AttesterSlashing::Electra(inner) => AttesterSlashingRef::Electra(inner),
    });
    process_attester_slashings(
        &mut beaconstate,
        iter,
        VerifySignatures::False,
        &mut ctxt,
        &spec,
    )?;

    Ok(())
}
