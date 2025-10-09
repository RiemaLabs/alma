use failure::{bail, Error};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

// Create size-based bins for a corpus directory by making symlink views.
// Returns a map from bin label -> directory path.
// Bins: small (<= p33), medium (p33..=p66), large (> p66)
pub fn ensure_size_bins(
    target_name: &str,
    corpus_dir: &Path,
    workspace_dir: &Path,
) -> Result<HashMap<String, PathBuf>, Error> {
    let mut entries: Vec<(PathBuf, u64)> = Vec::new();
    for entry in fs::read_dir(corpus_dir)? {
        let e = entry?;
        if e.file_type()?.is_file() {
            let meta = e.metadata()?;
            entries.push((e.path(), meta.len()));
        }
    }
    if entries.is_empty() {
        bail!(format!("no corpus files found in {}", corpus_dir.display()));
    }
    entries.sort_by_key(|(_, sz)| *sz);
    let n = entries.len();
    let idx33 = (n as f64 * 0.33).floor() as usize;
    let idx66 = (n as f64 * 0.66).floor() as usize;

    let bins_root = workspace_dir.join("rl_bins").join(target_name);
    fs::create_dir_all(&bins_root)?;

    let mut mkbin = |label: &str| -> io::Result<PathBuf> {
        let p = bins_root.join(label);
        fs::create_dir_all(&p)?;
        Ok(p)
    };

    let small_dir = mkbin("small")?;
    let medium_dir = mkbin("medium")?;
    let large_dir = mkbin("large")?;

    // Clear existing symlinks (best-effort)
    let _ = clear_dir_symlinks(&small_dir);
    let _ = clear_dir_symlinks(&medium_dir);
    let _ = clear_dir_symlinks(&large_dir);

    for (i, (p, _sz)) in entries.iter().enumerate() {
        let dest_dir = if i <= idx33 {
            &small_dir
        } else if i <= idx66 {
            &medium_dir
        } else {
            &large_dir
        };
        let file_name = p.file_name().map(|s| s.to_owned()).unwrap_or_default();
        let dest = dest_dir.join(file_name);
        // Create symlink; if fails on non-unix, fall back to copy
        #[cfg(unix)]
        {
            match unix_fs::symlink(&p, &dest) {
                Ok(_) => {}
                Err(_) => {
                    let _ = fs::copy(&p, &dest);
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = fs::copy(&p, &dest);
        }
    }

    let mut out = HashMap::new();
    out.insert("small".to_string(), small_dir);
    out.insert("medium".to_string(), medium_dir);
    out.insert("large".to_string(), large_dir);
    Ok(out)
}

fn clear_dir_symlinks(dir: &Path) -> io::Result<()> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        let _ = fs::remove_file(p);
    }
    Ok(())
}

pub fn sample_batch(corpus_src: &Path, batch_out: &Path, max_files: usize) -> Result<usize, Error> {
    fs::create_dir_all(batch_out)?;
    let mut entries: Vec<PathBuf> = Vec::new();
    for e in fs::read_dir(corpus_src)? {
        let p = e?.path();
        if p.is_file() {
            entries.push(p);
        }
    }
    // stratify by size terciles if possible
    let mut sized: Vec<(PathBuf, u64)> = Vec::new();
    for p in entries {
        let sz = p.metadata()?.len();
        sized.push((p, sz));
    }
    if sized.is_empty() {
        return Ok(0);
    }
    sized.sort_by_key(|(_, sz)| *sz);
    let n = sized.len();
    let idx33 = (n as f64 * 0.33).floor() as usize;
    let idx66 = (n as f64 * 0.66).floor() as usize;
    let mut picks = Vec::new();
    let per_bin = std::cmp::max(1, max_files / 3);
    // helper to push from a slice evenly spaced
    let mut push_even = |slice: &[(PathBuf, u64)]| {
        if slice.is_empty() {
            return;
        }
        let step = std::cmp::max(1, slice.len() / per_bin);
        let mut i = 0usize;
        while i < slice.len() && picks.len() < max_files {
            picks.push(slice[i].0.clone());
            i += step;
        }
    };
    push_even(&sized[..=idx33.min(n - 1)]);
    if idx66 > idx33 {
        push_even(&sized[idx33 + 1..=idx66.min(n - 1)]);
    }
    if idx66 + 1 < n {
        push_even(&sized[idx66 + 1..]);
    }
    // materialize as symlinks or copies
    let mut count = 0usize;
    for p in picks.into_iter() {
        let name = p.file_name().unwrap().to_owned();
        let dest = batch_out.join(name);
        #[cfg(unix)]
        {
            if unix_fs::symlink(&p, &dest).is_ok() {
                count += 1;
                continue;
            }
        }
        if fs::copy(&p, &dest).is_ok() {
            count += 1;
        }
        if count >= max_files {
            break;
        }
    }
    Ok(count)
}

pub fn prune_dir_by_hash_limit(dir: &Path, keep_limit: usize) -> Result<usize, Error> {
    // keep first occurrence per content hash, then cap total by size-diversity
    let mut files: Vec<PathBuf> = Vec::new();
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_file() {
            files.push(p);
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut uniq: Vec<(PathBuf, u64)> = Vec::new();
    for p in files.iter() {
        if let Ok(mut f) = fs::File::open(p) {
            let mut hasher = Sha256::new();
            let _ = std::io::copy(&mut f, &mut hasher);
            let h = hasher.finalize();
            if seen.insert(h) {
                let sz = p.metadata()?.len();
                uniq.push((p.clone(), sz));
            }
        }
    }
    // cap by size diversity buckets
    uniq.sort_by_key(|(_, sz)| *sz);
    let n = uniq.len();
    if n <= keep_limit {
        return Ok(n);
    }
    let step = std::cmp::max(1, n / keep_limit);
    let mut keep_set = std::collections::HashSet::new();
    let mut i = 0usize;
    while i < n && keep_set.len() < keep_limit {
        keep_set.insert(uniq[i].0.clone());
        i += step;
    }
    // remove others
    let mut removed = 0usize;
    for (p, _) in uniq.into_iter() {
        if !keep_set.contains(&p) {
            let _ = fs::remove_file(p);
            removed += 1;
        }
    }
    Ok(keep_limit)
}

pub fn merge_hfuzz_and_corpora_then_prune(
    workspace_dir: &Path,
    target_name: &str,
    corpora_label: &str,
    keep_limit: usize,
) -> Result<PathBuf, Error> {
    let corpora_src = workspace_dir.join("corpora").join(corpora_label);
    let hfuzz_in = workspace_dir
        .join("hfuzz")
        .join("hfuzz_workspace")
        .join(target_name)
        .join("input");
    let merge_dir = workspace_dir.join("rl_merge").join(target_name);
    fs::create_dir_all(&merge_dir)?;
    // copy corpora
    if corpora_src.exists() {
        for e in fs::read_dir(&corpora_src)? {
            let p = e?.path();
            if p.is_file() {
                let _ = fs::copy(&p, merge_dir.join(p.file_name().unwrap()));
            }
        }
    }
    // copy hfuzz inputs
    if hfuzz_in.exists() {
        for e in fs::read_dir(&hfuzz_in)? {
            let p = e?.path();
            if p.is_file() {
                let _ = fs::copy(&p, merge_dir.join(p.file_name().unwrap()));
            }
        }
    }
    // prune merged dir
    let _kept = prune_dir_by_hash_limit(&merge_dir, keep_limit)?;
    // write to corpora_pruned/<label>
    let out = workspace_dir.join("corpora_pruned").join(corpora_label);
    fs::create_dir_all(&out)?;
    // clear out then copy
    for e in fs::read_dir(&out)? {
        let p = e?.path();
        let _ = fs::remove_file(p);
    }
    for e in fs::read_dir(&merge_dir)? {
        let p = e?.path();
        if p.is_file() {
            let _ = fs::copy(&p, out.join(p.file_name().unwrap()));
        }
    }
    Ok(out)
}

pub fn merge_hfuzz_and_corpora_then_prune_into(
    workspace_dir: &Path,
    target_name: &str,
    corpora_label: &str,
    keep_limit: usize,
    out_root: &Path,
) -> Result<PathBuf, Error> {
    let corpora_src = workspace_dir.join("corpora").join(corpora_label);
    let hfuzz_in = workspace_dir
        .join("hfuzz")
        .join("hfuzz_workspace")
        .join(target_name)
        .join("input");
    let merge_dir = workspace_dir.join("rl_merge").join(target_name);
    fs::create_dir_all(&merge_dir)?;
    // copy corpora
    if corpora_src.exists() {
        for e in fs::read_dir(&corpora_src)? {
            let p = e?.path();
            if p.is_file() {
                let _ = fs::copy(&p, merge_dir.join(p.file_name().unwrap()));
            }
        }
    }
    // copy hfuzz inputs
    if hfuzz_in.exists() {
        for e in fs::read_dir(&hfuzz_in)? {
            let p = e?.path();
            if p.is_file() {
                let _ = fs::copy(&p, merge_dir.join(p.file_name().unwrap()));
            }
        }
    }
    // prune merged dir
    let _kept = prune_dir_by_hash_limit(&merge_dir, keep_limit)?;
    // write to provided out_root/<label>
    let out = out_root.join(corpora_label);
    fs::create_dir_all(&out)?;
    // clear out then copy
    for e in fs::read_dir(&out)? {
        let p = e?.path();
        let _ = fs::remove_file(p);
    }
    for e in fs::read_dir(&merge_dir)? {
        let p = e?.path();
        if p.is_file() {
            let _ = fs::copy(&p, out.join(p.file_name().unwrap()));
        }
    }
    Ok(out)
}
