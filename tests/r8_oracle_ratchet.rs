//! R8 mapping file as a TEST-TIME PRECISION RATCHET.
//!
//! The mapping is **never** read by production droidsaw. Real APKs
//! ship without `mapping.txt` by design (the whole point of
//! obfuscation), so any production decompile path that depended on a
//! mapping would be unfalsifiable on real corpora. This test
//! parses the corpus mapping files and uses them as an INDEPENDENT
//! ORACLE to catch FPs/FNs in the structural recognisers in
//! [`droidsaw_dex::r8_inversion`]:
//!
//! - **FP check**: every `@droidsaw R8Origin(<variant>, helper=...)`
//!   marker emitted by decompilation must name a helper class that
//!   R8's own mapping declares synthesized
//!   (`com.android.tools.r8.synthesized` annotation). A marker on a
//!   class R8 says is NOT synthesized is a structural false positive.
//!
//! - **FN check**: every class R8's mapping declares synthesized
//!   that ALSO matches the recogniser's preconditions (renamed
//!   namespace + ≥ 2 invoke-static callers + single-BB body + ≥ 2
//!   trampoline-shaped callers) should emit at least one marker.
//!   A synthesized class that meets the preconditions but produces
//!   no marker is a structural false negative.
//!
//! The two checks are not symmetric. FP is a hard assertion (any
//! false positive trips the test). FN is reported but not asserted
//! today — synthesized classes that fail the structural
//! preconditions are legitimate non-recognitions (e.g. R8
//! synthesized a helper whose body is multi-BB, so
//! `BlockOutlinedHelper` correctly declines).
//!
//! Skips cleanly when the fixture corpus hasn't been built (the
//! `corpus_r8_check` test populates `target/corpus-r8-out/`; run
//! that first or accept a skip).

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use common::r8_canonical_marker::{descriptor_to_mapping_key, is_valid_class_descriptor};
use proguard::ProguardRecord;

/// Maximum mapping file size accepted by the ratchet. A larger
/// file is rejected before any read into memory. Sized to admit
/// real R8 mappings on large-codebase apps (single-digit-MiB to
/// low-hundreds-of-MiB range) while refusing pathologically large
/// crafted input.
const MAX_MAPPING_BYTES: u64 = 256 * 1024 * 1024;

/// Maximum number of `<original> -> <obfuscated>:` class-rename
/// records accepted from a single mapping file. Real R8 output on
/// large APKs lands in the 10k–100k range; 1M is well beyond that
/// and signals a crafted input.
const MAX_RENAMED_CLASSES: usize = 1_000_000;

/// Maximum line distance between a `<class> -> <obf>:` declaration
/// and a `# {"id":"com.android.tools.r8.synthesized"}` annotation
/// that attributes to it. R8 emits annotations within a handful of
/// lines of their anchor; widening the proximity admits
/// mis-attribution via interleaved records.
const SYNTHESIZED_PROXIMITY_LINES: usize = 16;

/// The JSON-id literal R8 writes into the mapping for compiler-
/// introduced synthetic classes / members. Matching on this string
/// is the v2.2-spec contract; it does not depend on the wrapping
/// crate's enum naming.
const SYNTHESIZED_ID_LITERAL: &str = "com.android.tools.r8.synthesized";

/// Parsed mapping data the ratchet needs. Two queries:
///
/// 1. `is_synthesized(obfuscated_class)` — class is annotated by R8
///    with `com.android.tools.r8.synthesized`.
/// 2. `renamed_classes()` — iterator over every `<original> -> <obf>`
///    rename pair the mapping records.
#[derive(Debug, Default)]
struct OracleMapping {
    synthesized_classes: BTreeSet<String>,
    renamed_classes: Vec<(String, String)>,
    /// Obfuscated class names that appear more than once across the
    /// mapping. R8 maps distinct top-level classes to distinct
    /// obfuscated names; a duplicate is either a malformed input or
    /// a crafted attack to make the `synthesized_classes` set ambiguous
    /// (the `is_synthesized("a")` lookup cannot distinguish two
    /// different `<original> -> a:` records). The ratchet fails
    /// loudly when this set is non-empty rather than silently choosing
    /// one of the originals.
    duplicate_obfuscated: BTreeSet<String>,
    /// Number of synthesized annotations whose anchor class was
    /// MORE than `SYNTHESIZED_PROXIMITY_LINES` away. Surfaces as a
    /// warning counter in the ratchet output so a future R8 version
    /// that widens its annotation cadence is visible rather than
    /// silently dropping coverage.
    proximity_dropped: usize,
}

impl OracleMapping {
    /// Parse a mapping from `&str`. Two passes are interleaved across
    /// one iteration of `text.lines()`:
    ///
    /// 1. Synthesised-class detection — direct match on the
    ///    R8 v2.2 JSON literal `com.android.tools.r8.synthesized`
    ///    appearing on a `# ...` comment line within
    ///    `SYNTHESIZED_PROXIMITY_LINES` of its anchor class
    ///    declaration. Independent of the wrapping crate's enum
    ///    naming.
    /// 2. Class rename collection — feeds the same line through
    ///    `proguard::ProguardRecord::try_parse` for the structured
    ///    `Class { original, obfuscated }` variant.
    ///
    /// Bounded: at most `MAX_RENAMED_CLASSES` rename records are
    /// accepted. Beyond the cap the parse returns the partial result
    /// so the caller can still gate FP/FN checks against what was
    /// parsed before the bound tripped.
    fn parse(text: &str) -> Self {
        let mut synthesized_classes = BTreeSet::new();
        let mut renamed_classes = Vec::new();
        let mut duplicate_obfuscated = BTreeSet::new();
        let mut seen_obfuscated: BTreeSet<String> = BTreeSet::new();
        let mut proximity_dropped = 0usize;
        let mut current_class: Option<String> = None;
        let mut anchor_line: Option<usize> = None;
        for (line_no, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') && trimmed.contains(SYNTHESIZED_ID_LITERAL) {
                if let (Some(c), Some(anchor)) = (&current_class, anchor_line) {
                    if line_no.saturating_sub(anchor) <= SYNTHESIZED_PROXIMITY_LINES {
                        synthesized_classes.insert(c.clone());
                    } else {
                        proximity_dropped = proximity_dropped.saturating_add(1);
                    }
                }
                continue;
            }
            if let Ok(ProguardRecord::Class { original, obfuscated }) =
                ProguardRecord::try_parse(line.as_bytes())
            {
                let obf = obfuscated.to_string();
                if !seen_obfuscated.insert(obf.clone()) {
                    duplicate_obfuscated.insert(obf.clone());
                }
                current_class = Some(obf.clone());
                anchor_line = Some(line_no);
                if renamed_classes.len() < MAX_RENAMED_CLASSES {
                    renamed_classes.push((original.to_string(), obf));
                } else if renamed_classes.len() == MAX_RENAMED_CLASSES {
                    // Cap tripped — log once so the partial-parse
                    // outcome is visible. The Vec sticks at the cap;
                    // FN checks against renames beyond the cap will
                    // spuriously report "no marker for synthesized X."
                    eprintln!(
                        "WARN: OracleMapping::parse hit MAX_RENAMED_CLASSES={MAX_RENAMED_CLASSES}; further rename records ignored. Mapping is unusually large or crafted."
                    );
                }
            }
        }
        Self {
            synthesized_classes,
            renamed_classes,
            duplicate_obfuscated,
            proximity_dropped,
        }
    }

    /// Read + parse a mapping file from disk. Rejects:
    ///
    /// - Symbolic links (CI-poisoning vector — swap mapping.txt for
    ///   `/etc/passwd` or similar). The check uses
    ///   `fs::symlink_metadata` so the symlink target's content is
    ///   never read. Hardlinks share an inode so the check does NOT
    ///   detect them; defending against hardlink-swap requires
    ///   inode-ownership checks outside the scope of this ratchet
    ///   (the threat model accepts a hardlink as in-band attacker
    ///   input).
    /// - Files larger than `MAX_MAPPING_BYTES` (resource-exhaustion
    ///   class — crafted multi-GB mapping OOMs the test runner).
    fn from_file(path: &Path) -> std::io::Result<Self> {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("refusing to read mapping at symlink: {}", path.display()),
            ));
        }
        if metadata.len() > MAX_MAPPING_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "mapping file too large: {} bytes > {MAX_MAPPING_BYTES} byte cap ({})",
                    metadata.len(),
                    path.display(),
                ),
            ));
        }
        let text = std::fs::read_to_string(path)?;
        Ok(Self::parse(&text))
    }

    fn is_synthesized(&self, obfuscated_class: &str) -> bool {
        self.synthesized_classes.contains(obfuscated_class)
    }

    fn renamed_classes(&self) -> &[(String, String)] {
        &self.renamed_classes
    }

    fn duplicate_obfuscated(&self) -> &BTreeSet<String> {
        &self.duplicate_obfuscated
    }

    fn proximity_dropped(&self) -> usize {
        self.proximity_dropped
    }
}

/// Walk `target/corpus-r8-out/r8-9.0-release/` for `(classes.dex,
/// mapping.txt)` pairs.
fn corpus_pairs() -> std::io::Result<Vec<(String, PathBuf, PathBuf)>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("corpus-r8-out")
        .join("r8-9.0-release");
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let dex = entry.path().join("dex").join("classes.dex");
        let mapping = entry.path().join("dex").join("mapping.txt");
        if dex.exists() && mapping.exists() {
            out.push((name, dex, mapping));
        }
    }
    out.sort();
    Ok(out)
}

/// Marker frame literal. Recogniser-emitted markers always have
/// this exact prefix; matching on it (rather than free-scanning for
/// `helper=`) keeps DEX content with stray `helper=` substrings —
/// methods named `helper=La;->`, string literals, comment text in
/// source files — out of the ratchet's input.
const MARKER_FRAME_PREFIX: &str = "/* @droidsaw R8Origin(";
const MARKER_FRAME_SUFFIX: &str = ") */";

/// Pull every `helper=<class>-><method>` reference out of the
/// decompile output. Recognises ONLY markers whose entire line
/// matches the canonical R8Origin shape:
///
///   `/* @droidsaw R8Origin(<variant>, helper=<L...;>-><name>, callers=<N>, confidence=<N>) */`
///
/// The strict shape — fixed field order, named fields, numeric
/// `callers=N` / `confidence=N`, valid DEX class descriptor — admits
/// only emit-generated markers. A user-controlled string literal,
/// method name, or source comment cannot satisfy the full pattern
/// while ALSO appearing at line start (string literals are wrapped
/// in `"..."`; method names embedded in declarations begin with
/// `public`/`private`/access modifiers). Descriptors that fail
/// validation are silently dropped.
fn extract_helper_class_descriptors(decompiled: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in decompiled.lines() {
        let trimmed = line.trim();
        let Some(after_prefix) = trimmed.strip_prefix(MARKER_FRAME_PREFIX) else {
            continue;
        };
        let Some(frame_body) = after_prefix.strip_suffix(MARKER_FRAME_SUFFIX) else {
            continue;
        };
        let Some(desc) = parse_marker_body(frame_body) else {
            continue;
        };
        if is_valid_class_descriptor(desc) {
            out.insert(desc.to_string());
        }
    }
    out
}

/// Parse the body of a `/* @droidsaw R8Origin(<body>) */` marker
/// and return the helper descriptor if the body matches the strict
/// canonical shape. Returns `None` for any deviation — extra fields,
/// missing fields, out-of-order fields, non-numeric counters,
/// non-DEX-descriptor helpers, anything that isn't exactly what the
/// emit path renders.
fn parse_marker_body(body: &str) -> Option<&str> {
    // Canonical shape (split on ", "):
    //   <variant>, helper=<L...;>-><name>, callers=<N>, confidence=<N>
    let parts: Vec<&str> = body.split(", ").collect();
    if parts.len() != 4 {
        return None;
    }
    // parts[0] = variant name (e.g. "BlockOutlinedTrampoline")
    if !parts[0]
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    // parts[1] = `helper=<L...;>-><name>`
    let helper_field = parts[1].strip_prefix("helper=")?;
    let (desc, name) = helper_field.split_once("->")?;
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '<' || c == '>' || c == '$')
    {
        return None;
    }
    // parts[2] = `callers=<digits>`
    let callers_field = parts[2].strip_prefix("callers=")?;
    if !callers_field.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // parts[3] = `confidence=<digits>`
    let confidence_field = parts[3].strip_prefix("confidence=")?;
    if !confidence_field.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(desc)
}

#[test]
fn r8_oracle_ratchet_fp_check_synthesized_only() {
    let pairs = corpus_pairs().expect("walk corpus directory");
    if pairs.is_empty() {
        eprintln!(
            "SKIP: corpus not built. Run `cargo test --test corpus_r8_check` first."
        );
        return;
    }

    let mut total_markers = 0usize;
    let mut fp_findings: Vec<String> = Vec::new();

    let mut duplicate_findings: Vec<String> = Vec::new();
    let mut total_proximity_dropped = 0usize;

    for (fixture, dex_path, mapping_path) in &pairs {
        let dex_bytes = std::fs::read(dex_path).expect("read dex");
        let oracle = OracleMapping::from_file(mapping_path).expect("read mapping");
        // Crafted-mapping defense: a mapping that records the same
        // obfuscated name twice with different originals would make
        // `is_synthesized("a")` ambiguous (one original synthesized,
        // one not — the lookup returns whichever set membership wins).
        // R8 does not emit duplicates on legitimate input; non-empty
        // here means malformed or adversarial.
        for obf in oracle.duplicate_obfuscated() {
            duplicate_findings.push(format!(
                "{fixture}: mapping records obfuscated name {obf:?} more than once (malformed or crafted input)",
            ));
        }
        total_proximity_dropped =
            total_proximity_dropped.saturating_add(oracle.proximity_dropped());
        let dex = match droidsaw_dex::parser::DexFile::parse(&dex_bytes, None) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("WARN: {fixture}: parse failed: {e:?}");
                continue;
            }
        };
        for class_def in &dex.class_defs {
            if class_def.class_data_off == 0 {
                continue;
            }
            let decompiled =
                droidsaw_dex::classes::decompile_class(&dex, &dex_bytes, class_def);
            let markers = extract_helper_class_descriptors(&decompiled);
            total_markers = total_markers.saturating_add(markers.len());
            for helper_desc in &markers {
                let key = descriptor_to_mapping_key(helper_desc);
                if !oracle.is_synthesized(&key) {
                    fp_findings.push(format!(
                        "{fixture}: marker references helper {helper_desc} (mapping key {key:?}) which R8 does NOT declare com.android.tools.r8.synthesized",
                    ));
                }
            }
        }
    }

    eprintln!(
        "RATCHET FP SUMMARY: {} fixtures scanned, {} markers checked, {} FPs, {} duplicate-obfuscated, {} proximity-dropped synthesized annotations",
        pairs.len(),
        total_markers,
        fp_findings.len(),
        duplicate_findings.len(),
        total_proximity_dropped,
    );

    assert!(
        duplicate_findings.is_empty(),
        "mapping contains duplicate obfuscated names — malformed or crafted input:\n  - {}",
        duplicate_findings.join("\n  - "),
    );
    assert!(
        fp_findings.is_empty(),
        "structural recognisers fired on classes R8 does not declare synthesized:\n  - {}",
        fp_findings.join("\n  - "),
    );
}

#[test]
fn r8_oracle_ratchet_fn_report_synthesized_without_markers() {
    let pairs = corpus_pairs().expect("walk corpus directory");
    if pairs.is_empty() {
        eprintln!(
            "SKIP: corpus not built. Run `cargo test --test corpus_r8_check` first."
        );
        return;
    }

    let mut report: Vec<String> = Vec::new();
    for (fixture, dex_path, mapping_path) in &pairs {
        let dex_bytes = std::fs::read(dex_path).expect("read dex");
        let oracle = OracleMapping::from_file(mapping_path).expect("read mapping");
        let dex = match droidsaw_dex::parser::DexFile::parse(&dex_bytes, None) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("WARN: {fixture}: parse failed: {e:?}");
                continue;
            }
        };
        let mut fired_helpers: BTreeSet<String> = BTreeSet::new();
        for class_def in &dex.class_defs {
            if class_def.class_data_off == 0 {
                continue;
            }
            let decompiled =
                droidsaw_dex::classes::decompile_class(&dex, &dex_bytes, class_def);
            for desc in extract_helper_class_descriptors(&decompiled) {
                fired_helpers.insert(descriptor_to_mapping_key(&desc));
            }
        }
        for (original, obfuscated) in oracle.renamed_classes() {
            if !oracle.is_synthesized(obfuscated) {
                continue;
            }
            if fired_helpers.contains(obfuscated) {
                continue;
            }
            report.push(format!(
                "{fixture}: mapping declares {obfuscated} (original {original}) synthesized but no marker references it",
            ));
        }
    }

    if !report.is_empty() {
        eprintln!("RATCHET FN REPORT (informational, not asserted):");
        for r in &report {
            eprintln!("  - {r}");
        }
    } else {
        eprintln!("RATCHET FN REPORT: all synthesized classes have markers");
    }
    // FN is informational. Many synthesized classes legitimately fail
    // the recogniser's structural preconditions (multi-BB helper
    // bodies, single-caller helpers, helpers with try regions); the
    // recogniser correctly declines those. Asserting FN parity here
    // would gate the test on the recogniser's coverage of every
    // synthesised shape, which is not the recogniser's contract.
}
