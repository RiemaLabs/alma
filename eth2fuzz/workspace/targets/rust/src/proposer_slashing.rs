use state_processing::{
    per_block_processing::process_operations::process_proposer_slashings,
    ConsensusContext, BlockProcessingError, VerifySignatures,
};

use types::{BeaconState, EthSpec, MainnetEthSpec, ProposerSlashing, RelativeEpoch};

    /// Run `process_proposer_slashings`
pub fn process_proposer_slashing(
    mut beaconstate: BeaconState<MainnetEthSpec>,
    proposer_slashing: ProposerSlashing,
) -> Result<(), BlockProcessingError> {
    let spec = MainnetEthSpec::default_spec();
    //let mut state = &mut self.pre;
    // Ensure the current epoch cache is built.
    // Required by slash_validator->initiate_validator_exit->get_churn_limit
    beaconstate.build_committee_cache(RelativeEpoch::Current, &spec)?;

    let mut ctxt = ConsensusContext::new(beaconstate.slot());
    process_proposer_slashings(
        &mut beaconstate,
        &[proposer_slashing],
        VerifySignatures::False,
        &mut ctxt,
        &spec,
    )?;

    Ok(())
}
