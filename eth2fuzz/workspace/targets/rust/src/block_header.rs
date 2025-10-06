use state_processing::{
    per_block_processing::process_block_header,
    ConsensusContext, VerifyBlockRoot,
};

use types::{BeaconBlock, BeaconState, ChainSpec, EthSpec, MainnetEthSpec};

/// Run `process_block_header`
pub fn process_header(
    mut beaconstate: BeaconState<MainnetEthSpec>,
    block: BeaconBlock<MainnetEthSpec>,
) -> Result<(), ()> {
    let spec: ChainSpec = MainnetEthSpec::default_spec();
    // Skip if slots mismatch.
    if beaconstate.slot() != block.slot() {
        return Ok(());
    }
    let mut ctxt = ConsensusContext::new(beaconstate.slot());
    let _ = process_block_header(
        &mut beaconstate,
        block.temporary_block_header(),
        VerifyBlockRoot::True,
        &mut ctxt,
        &spec,
    );
    Ok(())
}
