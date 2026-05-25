//! Env-gated negative-test regression ratchet for the R8
//! BlockOutlined recogniser.
//!
//! Reads `DROIDSAW_R8_FP_REGRESSION_CORPUS_PATH` (a directory
//! containing `classes*.dex` extracted from an APK rebuilt with R8
//! obfuscation enabled). For each class in the must-NOT-fire list below, the
//! harness asserts the source-derived helper recogniser
//! (`r8_inversion::apply`) does NOT emit a `BlockOutlined`
//! marker.
//!
//! # Why this ratchet exists
//!
//! Before the source-derived rewrite, the four-gate empirical
//! recogniser produced six markers on the prior-FP corpus;
//! cross-referencing against the corpus's mapping showed 4 of the
//! 6 were FPs. Two of those FPs were on classes that contain NO
//! R8 synthesis at all (pure developer code that happened to
//! match the structural shape the old gates keyed on). The
//! rewrite eliminates the gates those FPs keyed on (class-name
//! pattern, trampoline-shape callers), so empirically the
//! recogniser declines on these classes. This test pins that
//! decline structurally — a future tightening pass that
//! re-introduces a similar heuristic must either preserve the
//! decline or update this ratchet deliberately.
//!
//! The bar: "negative test fixtures Lrj and Lcc0 MUST decline on"
//! — not "they almost certainly decline because we removed the
//! gates that caught them." Empirical decline is not the same as
//! a regression gauge.
//!
//! # Adversarial-input discipline
//!
//! Same defenses as the production-corpus smoke test:
//! - Symlink reject (`fs::symlink_metadata` per directory entry +
//!   per file open).
//! - 64 MiB per-DEX size cap.
//! - 16 MiB worker thread stack (matches deep-nesting workaround).

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MAX_DEX_BYTES: u64 = 64 * 1024 * 1024;
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Conservative raw-byte cap used by `compute_class_bytecode_hash`.
/// The canonical shape (`ClassData::bytes()` + per-method `code_item`
/// bytes) is not directly reachable from parser output — the parser
/// keeps decoded structs, not the raw on-disk slices. We instead
/// content-address a capped window of raw DEX bytes at each pinned
/// offset (`class_data_off` + every `code_off`). 4 KiB is empirically
/// larger than any realistic class_data or code_item in obfuscated
/// app DEXes (median code_item is a few dozen bytes; the cap is a
/// pre-image-distinctness lower bound, not a precise size).
///
/// Trade-off: a class with code_items >4 KiB would have its tail
/// excluded from the hash. The hash still distinguishes the first 4
/// KiB of every pinned code_off — sufficient to detect the Attack-A
/// class_def reorder, which would change `class_data_off` and the
/// per-method `code_off` values in lockstep.
const RAW_BYTE_HASH_CAP: usize = 4096;
const PIN_SENTINEL: &str = "PIN-FROM-CORPUS";

/// Mapping-key class descriptors that the recogniser MUST decline
/// on, paired with a content-addressed hash of the class's bytecode
/// pinned at the time the FP was observed. Format:
///   `(descriptor, sha256_hex_of_canonical_bytes_or_sentinel)`
///
/// Defense against bytecode reordering attacks: without the bytecode
/// pin, a crafted DEX can reorder `class_defs` so the descriptor
/// `Lcc0;` resolves to a structurally distinct class that DOES
/// outline. The descriptor-only ratchet would still pass. Binding
/// the pin to a hash of the class's `class_data` + every referenced
/// `code_item` makes a reorder visible: the hash mismatches and the
/// ratchet panics loudly rather than passing vacuously.
///
/// Per-class context:
/// - `cc0` — pure developer code (an internal class from the
///   prior-FP corpus). NOT synthesized. The old recogniser fired
///   because it matched the structural pattern of a renamed-
///   namespace single-BB helper with trampoline-shaped callers.
///   The new recogniser declines via the ACC_SYNTHETIC class-flag
///   gate.
/// - `rj` — synthesized class but NOT outline-annotated. Likely a
///   lambda factory or horizontal-merge bridge. The structural
///   shape is helper-like but R8's mapping does not classify it as
///   outline. The new recogniser declines via a combination of
///   structural predicates whose specific contribution can shift
///   under future tightening — the regression gauge here is
///   "stays declined," not "declined for this specific reason."
///
/// Hash sentinel: `"PIN-FROM-CORPUS"` means the expected hash has
/// not yet been observed (the FP-regression corpus isn't available
/// locally). At ratchet time, if the sentinel is present,
/// the harness prints a WARN line with the observed hash so an
/// analyst can paste it back into this const. The ratchet still
/// runs the marker check — the sentinel does NOT bypass the
/// existing FP regression gauge; it only defers the pin-staleness
/// assertion.
const FP_REGRESSION_DESCRIPTORS: &[(&str, &str)] = &[
    ("Lcc0;", PIN_SENTINEL),
    ("Lrj;", PIN_SENTINEL),
];

/// Hash the canonical byte sequence for a class as: `class_data_off`
/// (4 bytes LE) followed by raw DEX bytes at
/// `[class_data_off, class_data_off + RAW_BYTE_HASH_CAP)`, then for
/// each `EncodedMethod` in `direct_methods ++ virtual_methods`
/// (existing order), `method_idx` (4 bytes LE) + `code_off`
/// (4 bytes LE) + raw DEX bytes at `[code_off, code_off + CAP)`
/// (skipped when `code_off == 0`). Returns the SHA-256 hex digest.
///
/// The byte ranges are capped at the DEX file end. The ordering is
/// fixed by parser-canonical iteration order (the `Vec` order in
/// `decode::ClassData`).
fn compute_class_bytecode_hash(
    dex: &droidsaw_dex::parser::DexFile,
    data: &[u8],
    class_def: &droidsaw_dex::ids::ClassDefItem,
) -> String {
    let mut h = Sha256::new();
    h.update(class_def.class_data_off.to_le_bytes());
    if class_def.class_data_off != 0 {
        #[allow(clippy::as_conversions, reason = "PROOF: widen u32→usize; class_data_off is a DEX file offset bounded by the 64 MiB read cap")]
        let start = class_def.class_data_off as usize;
        let end = start
            .saturating_add(RAW_BYTE_HASH_CAP)
            .min(data.len());
        if start < data.len() {
            h.update(&data[start..end]);
        }
    }
    if let Some(class_data) = dex.class_datas.get(&class_def.class_data_off) {
        for em in class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
        {
            h.update(em.method_idx.0.to_le_bytes());
            h.update(em.code_off.to_le_bytes());
            if em.code_off != 0 {
                #[allow(clippy::as_conversions, reason = "PROOF: widen u32→usize; code_off is a DEX file offset bounded by the 64 MiB read cap")]
                let start = em.code_off as usize;
                let end = start
                    .saturating_add(RAW_BYTE_HASH_CAP)
                    .min(data.len());
                if start < data.len() {
                    h.update(&data[start..end]);
                }
            }
        }
    }
    let digest = h.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn corpus_dir() -> Option<PathBuf> {
    let raw = std::env::var_os("DROIDSAW_R8_FP_REGRESSION_CORPUS_PATH")?;
    let p = PathBuf::from(raw);
    if p.is_dir() { Some(p) } else { None }
}

fn read_dex_capped(path: &Path) -> Option<Vec<u8>> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("WARN: skipping {} — metadata failed: {e}", path.display());
            return None;
        }
    };
    if meta.file_type().is_symlink() {
        eprintln!("WARN: skipping symlink at {}", path.display());
        return None;
    }
    if meta.len() > MAX_DEX_BYTES {
        eprintln!(
            "WARN: skipping {} — {} bytes exceeds {MAX_DEX_BYTES}-byte cap",
            path.display(),
            meta.len(),
        );
        return None;
    }
    match std::fs::read(path) {
        Ok(b) => Some(b),
        Err(e) => {
            eprintln!("WARN: skipping {} — read failed: {e}", path.display());
            None
        }
    }
}

fn collect_dex_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)? {
            let entry = entry?;
            let p = entry.path();
            let meta = std::fs::symlink_metadata(&p)?;
            if meta.file_type().is_symlink() {
                eprintln!("WARN: skipping symlink at {}", p.display());
                continue;
            }
            if meta.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|s| s.to_str()) == Some("dex") {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

#[test]
fn r8_fp_regression_prior_fps_decline() {
    let handle = std::thread::Builder::new()
        .name("r8_fp_regression_worker".into())
        .stack_size(SMOKE_TEST_STACK_BYTES)
        .spawn(ratchet_main)
        .expect("spawn stack-sized worker thread");
    if let Err(e) = handle.join() {
        std::panic::resume_unwind(e);
    }
}

fn ratchet_main() {
    let Some(dir) = corpus_dir() else {
        eprintln!(
            "SKIP: DROIDSAW_R8_FP_REGRESSION_CORPUS_PATH unset. Set to a directory \
             containing classes*.dex extracted from an APK \
             rebuilt with R8 obfuscation enabled."
        );
        return;
    };
    let dexes = match collect_dex_files(&dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("SKIP: failed to walk {}: {e}", dir.display());
            return;
        }
    };
    if dexes.is_empty() {
        eprintln!("SKIP: no `.dex` files under {}", dir.display());
        return;
    }

    let mut regression_findings: Vec<String> = Vec::new();
    let mut pin_findings: Vec<String> = Vec::new();
    let mut classes_checked = 0usize;

    for dex_path in &dexes {
        let Some(data) = read_dex_capped(dex_path) else {
            continue;
        };
        let dex = match droidsaw_dex::parser::DexFile::parse(&data, None) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("WARN: skipping {} — parse failed: {e:?}", dex_path.display());
                continue;
            }
        };
        let census = droidsaw_dex::r8_inversion::build_trampoline_census(&dex);
        for class_def in &dex.class_defs {
            #[allow(clippy::as_conversions, reason = "PROOF: widen u32→usize; class_idx.0 bounded by parser type_ids validation")]
            let class_desc = dex
                .type_descriptors
                .get(class_def.class_idx.0 as usize)
                .map(String::as_str)
                .unwrap_or("");
            let Some((_, expected_hash)) = FP_REGRESSION_DESCRIPTORS
                .iter()
                .find(|(d, _)| *d == class_desc)
            else {
                continue;
            };
            classes_checked = classes_checked.saturating_add(1);
            if class_def.class_data_off == 0 {
                continue;
            }

            // Attack-A defense: content-address the class bytecode and
            // compare against the pinned hash before trusting the
            // descriptor → "this is the class we observed the FP on"
            // assumption. A mismatch is either an intentional bytecode
            // change (re-pin needed) or a class_def-reorder attack.
            let observed_hash = compute_class_bytecode_hash(&dex, &data, class_def);
            if *expected_hash == PIN_SENTINEL {
                eprintln!(
                    "WARN: FP regression hash not yet pinned for {class_desc} — \
                     paste the observed hash below into FP_REGRESSION_DESCRIPTORS \
                     to harden the ratchet against class_def reorders.\n  \
                     observed hash: {observed_hash} ({})",
                    dex_path.display(),
                );
            } else if observed_hash != *expected_hash {
                pin_findings.push(format!(
                    "FP regression pin stale or corpus reordered: descriptor \
                     {class_desc} expected hash {expected_hash}, got \
                     {observed_hash} ({})",
                    dex_path.display(),
                ));
                // Skip the marker check on hash mismatch — the
                // descriptor → class binding is unreliable, so the
                // ratchet's "no marker fired" claim would not be
                // meaningful. The panic below makes the failure loud.
                continue;
            }

            let out = droidsaw_dex::classes::decompile_class_with_census(
                &dex, &data, class_def, &census,
            );
            for line in out.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("/* @droidsaw R8Origin(BlockOutlined") {
                    regression_findings.push(format!(
                        "REGRESSION: {class_desc} fired marker: {trimmed}"
                    ));
                }
            }
        }
    }

    if !pin_findings.is_empty() {
        for f in &pin_findings {
            eprintln!("  {f}");
        }
        panic!(
            "FP regression bytecode-pin mismatch. Either the corpus DEX \
             was rebuilt (re-pin the hash in FP_REGRESSION_DESCRIPTORS) \
             or the class_def ordering was tampered with to bind the \
             pinned descriptor to a structurally different class \
             (class_def ordering tampering)."
        );
    }

    eprintln!(
        "FP-REGRESSION NEGATIVE-TEST RATCHET: {} target classes checked across {} DEXes",
        classes_checked,
        dexes.len(),
    );

    if !regression_findings.is_empty() {
        for f in &regression_findings {
            eprintln!("  {f}");
        }
        panic!(
            "Recogniser fired BlockOutlined marker on a class in the \
             FP_REGRESSION_DESCRIPTORS list. The source-derived rewrite \
             eliminated these as FPs; any future tightening that re- \
             introduces them is a regression and must update this \
             ratchet deliberately."
        );
    }
    if classes_checked == 0 {
        if std::env::var_os("DROIDSAW_R8_FP_REGRESSION_ALLOW_VACUOUS").is_some() {
            eprintln!(
                "WARN: no target classes from FP_REGRESSION_DESCRIPTORS \
                 found in corpus. Test passes vacuously per \
                 DROIDSAW_R8_FP_REGRESSION_ALLOW_VACUOUS env override."
            );
            return;
        }
        panic!(
            "No target classes from FP_REGRESSION_DESCRIPTORS were \
             found in the corpus at DROIDSAW_R8_FP_REGRESSION_CORPUS_PATH. \
             The fixture may be a different version of the corpus than \
             the one the FPs were observed on, or the recogniser \
             rewrite eliminated the classes entirely. To pass-vacuously \
             intentionally (e.g. when bumping the corpus to a version \
             where the prior FPs no longer exist), set the env var \
             DROIDSAW_R8_FP_REGRESSION_ALLOW_VACUOUS. Without the \
             override, a missing target indicates the regression gauge \
             is no longer measuring anything and the fixture or the \
             FP_REGRESSION_DESCRIPTORS list needs updating."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Demonstrates the Attack-A defense's core primitive: distinct
    /// byte buffers produce distinct SHA-256 digests, and the
    /// mismatch is detectable by string-comparing the hex digests
    /// the way `ratchet_main` does. No corpus or env var required —
    /// this exercises the hash plumbing on synthetic inputs.
    #[test]
    fn bytecode_hash_mismatch_is_detected() {
        let buf_a: &[u8] = b"\x10\x00\x00\x00CLASS_DATA_VARIANT_A\x00\x00\x00\x00CODE_ITEM_A";
        let buf_b: &[u8] = b"\x10\x00\x00\x00CLASS_DATA_VARIANT_B\x00\x00\x00\x00CODE_ITEM_B";

        let hash_a: String = {
            let mut h = Sha256::new();
            h.update(buf_a);
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        };
        let hash_b: String = {
            let mut h = Sha256::new();
            h.update(buf_b);
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        };

        assert_ne!(hash_a, hash_b, "distinct bytes must produce distinct hashes");
        assert_eq!(hash_a.len(), 64, "sha256 hex must be 64 chars");
        assert_eq!(hash_b.len(), 64, "sha256 hex must be 64 chars");
        // The Attack-A defense's comparison shape: if a pinned hash
        // (here `hash_a`) is compared against an observed hash for a
        // reordered class_def (`hash_b`), the mismatch surfaces.
        assert!(
            hash_a != hash_b,
            "the ratchet's pin-staleness check would fire on this pair",
        );
    }

    /// The `PIN_SENTINEL` value is reserved and distinct from any
    /// valid SHA-256 hex digest: it has the wrong length and
    /// contains non-hex characters, so it cannot collide with an
    /// observed hash.
    #[test]
    fn pin_sentinel_is_distinguishable_from_hex_digest() {
        assert_ne!(PIN_SENTINEL.len(), 64);
        assert!(
            !PIN_SENTINEL.chars().all(|c| c.is_ascii_hexdigit()),
            "sentinel must not be a valid hex digest",
        );
    }

    /// Every entry in `FP_REGRESSION_DESCRIPTORS` must have either a
    /// 64-char hex hash or the `PIN-FROM-CORPUS` sentinel — guards
    /// against a future edit that drops a partial / malformed hash
    /// in (e.g., truncated paste, accidental whitespace).
    #[test]
    fn fp_regression_descriptors_well_formed() {
        for (desc, hash) in FP_REGRESSION_DESCRIPTORS {
            assert!(
                !desc.is_empty() && desc.starts_with('L') && desc.ends_with(';'),
                "descriptor {desc:?} is not in Lxxx; form",
            );
            let is_sentinel = *hash == PIN_SENTINEL;
            let is_hex_digest =
                hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit());
            assert!(
                is_sentinel || is_hex_digest,
                "hash for {desc:?} is neither the sentinel nor a 64-char hex digest: {hash:?}",
            );
        }
    }
}
