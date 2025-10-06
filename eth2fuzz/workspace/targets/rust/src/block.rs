use state_processing::{
    per_block_processing, BlockSignatureStrategy, BlockProcessingError, ConsensusContext,
    VerifyBlockRoot,
};

use types::{BeaconState, ChainSpec, EthSpec, MainnetEthSpec, RelativeEpoch, SignedBeaconBlock};

    /// Run `process_block`
    /// TODO N convert to a library in lighthouse that we import
    /// Most of this is copied from lighthouse/ef_tests/src/cases/sanity_blocks.rs,
    /// as lighthouse doesn't directly implement the state_transition function.
pub fn state_transition(mut beaconstate: BeaconState<MainnetEthSpec>,
    block: SignedBeaconBlock<MainnetEthSpec>,
    validate_state_root: bool)
        -> Result<(), BlockProcessingError> {

    let spec: ChainSpec = MainnetEthSpec::default_spec();
    //let mut state = beaconstate; // No need to clone here, but means state_transition can only be called once?

    // TODO(gnattishness) any reason why we would want to unwrap and panic here vs returning an error?
    beaconstate.build_caches(&spec)?;
    let mut result = {
        /* not good for fuzzing (in some spectests cases, this can loop for 65k iteration )
        while state.slot < block.slot() {
            // TODO(gnattishness) handle option
            // requires implementation of an error trait that I can specify as the
            // return type
            per_slot_processing(&mut state, None, spec).unwrap();
        }
        */
        beaconstate
            .build_committee_cache(RelativeEpoch::Current, &spec)
            .unwrap();

        // Ensure the state's slot matches the block's slot; otherwise the
        // header check will fail in per_block_processing. Skip if mismatch.
        if beaconstate.slot() != block.slot() {
            return Ok(()); // skip this input
        }

        let mut ctxt = ConsensusContext::new(beaconstate.slot());
        per_block_processing(
            &mut beaconstate,
            &block,
            BlockSignatureStrategy::VerifyIndividual,
            VerifyBlockRoot::True,
            &mut ctxt,
            &spec,
        )?;
        beaconstate
    };

    if !validate_state_root
        || result
            .canonical_root()
            .ok()
            .map(|r| r == block.state_root())
            .unwrap_or(false)
    {
        Ok(())
    } else {
        Err(BlockProcessingError::StateRootMismatch)
    }
}
