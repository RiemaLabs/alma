/*
name: lodestar
github: https://github.com/ChainSafe/lodestar
npm: https://www.npmjs.com/package/@chainsafe/lodestar

NOTES: install Lodestar which includes @lodestar/types (phase0, altair, bellatrix, capella, deneb, electra)
npm i @chainsafe/lodestar
*/
// Load @lodestar/types for multiple forks (ESM only)
const _forks = {
    phase0: null,
    altair: null,
    bellatrix: null,
    capella: null,
    deneb: null,
    electra: null,
};
let _enrModule = null;
try {
    // Preload ESM modules asynchronously; cache when available
    import('@lodestar/types/phase0').then((m) => { _forks.phase0 = m.ssz; }).catch(() => {});
    import('@lodestar/types/altair').then((m) => { _forks.altair = m.ssz; }).catch(() => {});
    import('@lodestar/types/bellatrix').then((m) => { _forks.bellatrix = m.ssz; }).catch(() => {});
    import('@lodestar/types/capella').then((m) => { _forks.capella = m.ssz; }).catch(() => {});
    import('@lodestar/types/deneb').then((m) => { _forks.deneb = m.ssz; }).catch(() => {});
    import('@lodestar/types/electra').then((m) => { _forks.electra = m.ssz; }).catch(() => {});
    import('@chainsafe/enr').then((m) => { _enrModule = m; }).catch(() => {});
} catch (e) {
    // ignore
}

function getLoadedForks() {
    return Object.values(_forks).filter(Boolean);
}
// TODO - improve to not only fuzz ssz parsing
// but also processing function
// need to deal with beaconstate and config loading

// state-transition
// https://github.com/ChainSafe/lodestar/tree/master/packages/lodestar-beacon-state-transition


function is_lodestar_valid_exception(e)  {
    // Those are "valid" exceptions. 
    if (e.name == "Error" ) {} 
    // following condition are temporary
    // waiting for fix of https://github.com/ChainSafe/ssz/issues/23
    // waiting for fix of https://github.com/ChainSafe/ssz/issues/22
    // else if (e.message == "Offset is outside the bounds of the DataView" ) {} 
    // else if (e.message == "Cannot convert undefined to a BigInt" ) {} 
    else {
        throw e;
    }

}

function fuzz_lodestar_attestation(buf) {
    const forks = getLoadedForks();
    if (forks.length === 0) return;
    for (const ssz of forks) {
        if (!ssz.Attestation) continue;
        try { ssz.Attestation.deserialize(buf); } catch (e) { is_lodestar_valid_exception(e); }
    }
}

function fuzz_lodestar_attester_slashing(buf) {
    const forks = getLoadedForks();
    if (forks.length === 0) return;
    for (const ssz of forks) {
        if (!ssz.AttesterSlashing) continue;
        try { ssz.AttesterSlashing.deserialize(buf); } catch (e) { is_lodestar_valid_exception(e); }
    }
}


function fuzz_lodestar_block(buf) {
    const forks = getLoadedForks();
    if (forks.length === 0) return;
    for (const ssz of forks) {
        if (!ssz.BeaconBlock) continue;
        try { ssz.BeaconBlock.deserialize(buf); } catch (e) { is_lodestar_valid_exception(e); }
    }
}

function fuzz_lodestar_block_header(buf) {
    const forks = getLoadedForks();
    if (forks.length === 0) return;
    for (const ssz of forks) {
        if (!ssz.BeaconBlockHeader) continue;
        try { ssz.BeaconBlockHeader.deserialize(buf); } catch (e) { is_lodestar_valid_exception(e); }
    }
}
function fuzz_lodestar_deposit(buf) {
    const forks = getLoadedForks();
    if (forks.length === 0) return;
    for (const ssz of forks) {
        if (!ssz.Deposit) continue; // mainly phase0
        try { ssz.Deposit.deserialize(buf); } catch (e) { is_lodestar_valid_exception(e); }
    }
}
function fuzz_lodestar_proposer_slashing(buf) {
    const forks = getLoadedForks();
    if (forks.length === 0) return;
    for (const ssz of forks) {
        if (!ssz.ProposerSlashing) continue; // phase0
        try { ssz.ProposerSlashing.deserialize(buf); } catch (e) { is_lodestar_valid_exception(e); }
    }
}
function fuzz_lodestar_voluntary_exit(buf) {
    const forks = getLoadedForks();
    if (forks.length === 0) return;
    for (const ssz of forks) {
        if (!ssz.VoluntaryExit) continue; // phase0
        try { ssz.VoluntaryExit.deserialize(buf); } catch (e) { is_lodestar_valid_exception(e); }
    }
}

function fuzz_lodestar_beaconstate(buf) {
    const forks = getLoadedForks();
    if (forks.length === 0) return;
    for (const ssz of forks) {
        if (!ssz.BeaconState) continue;
        try { ssz.BeaconState.deserialize(buf); } catch (e) { is_lodestar_valid_exception(e); }
    }
}

// Test parsing ENR base64 encoded string
// install with
// npm i @chainsafe/enr
function fuzz_lodestar_enr(buf) {
    if (!_enrModule || !_enrModule.ENR) return; // module not yet available
    try {
        _enrModule.ENR.decodeTxt(buf.toString());
    } catch (e) {
        if (e.name == "Error") {}
        else { throw e; }
    }
}

module.exports = {
    fuzz_lodestar_attestation,
    fuzz_lodestar_attester_slashing,
    fuzz_lodestar_block,
    fuzz_lodestar_block_header,
    fuzz_lodestar_deposit,
    fuzz_lodestar_proposer_slashing,
    fuzz_lodestar_voluntary_exit,
    fuzz_lodestar_beaconstate,
    fuzz_lodestar_enr,
}
