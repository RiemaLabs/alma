use state_processing::{per_block_processing::process_operations::process_deposits, BlockProcessingError};

use types::{BeaconState, Deposit, EthSpec, MainnetEthSpec};

    /// Run `process_deposit`
pub fn process_deposit(
    mut beaconstate: BeaconState<MainnetEthSpec>,
    deposit: Deposit,
) -> Result<(), BlockProcessingError> {
    let spec = MainnetEthSpec::default_spec();

    // New Lighthouse expects processing deposits in batch with count checks.
    // We pass a single-element slice; this may frequently error on count mismatch,
    // which is fine for fuzzing purposes.
    process_deposits(&mut beaconstate, &[deposit], &spec)?;

    Ok(())
}
