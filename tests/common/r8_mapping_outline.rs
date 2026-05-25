//! Parser for R8's outline-specific mapping.txt annotations:
//! `com.android.tools.r8.outline` (outlined method body) and
//! `com.android.tools.r8.outlineCallsite` (caller of an outlined
//! helper, JSON-encoded with a `"outline"` field referencing the
//! helper method).
//!
//! Sibling to the synthesized-set parser at `r8_oracle_ratchet.rs`.
//! The `synthesized` annotation is a superset covering many R8
//! synthesis kinds (desugar, lambda factories, nest-mate bridges,
//! horizontal merges, outlines); the `outline` ID is the narrower
//! subset that identifies outlined methods specifically.
//!
//! # Hardening
//!
//! Input is analyst-supplied mapping.txt — adversarial-input
//! discipline matches `OracleMapping`:
//!
//! - File-size cap before any read (`MAX_MAPPING_BYTES`).
//! - Symlink reject via `fs::symlink_metadata`.
//! - Bounded proximity from anchor method to annotation comment
//!   (`OUTLINE_PROXIMITY_LINES`); annotations beyond increment a
//!   dropped-counter rather than mis-attaching.
//! - Bounded record count (`MAX_OUTLINE_METHODS`); cap-trip logs WARN.
//! - Duplicate-method-key detection: R8 maps distinct outlined
//!   methods to distinct (obfuscated_class, obfuscated_method)
//!   tuples; a duplicate is malformed or crafted input.
//!
//! Parses line-shapes directly (no `proguard` crate dep) so this
//! module is sharable across tests without crossing the test-only
//! discipline gate (`scripts/check-proguard-test-only.sh`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Per-file size cap before any read into memory. Sized to admit
/// real R8 mappings on large-codebase apps (single-digit-MiB to
/// low-hundreds-of-MiB range) while refusing pathologically large
/// crafted input. Matches `OracleMapping::MAX_MAPPING_BYTES`.
const MAX_MAPPING_BYTES: u64 = 256 * 1024 * 1024;

/// Maximum number of lines between an anchor method record and the
/// `com.android.tools.r8.outline` annotation that R8 emits below
/// it. Empirical: R8 emits the annotation 1-3 lines below the
/// method record. 16 is generous.
const OUTLINE_PROXIMITY_LINES: usize = 16;

/// Cap on outline-body records parsed. Real R8 output on production
/// APKs lands at 10-200 outlines per APK; 1M is well beyond.
const MAX_OUTLINE_METHODS: usize = 1_000_000;

/// Cap on outline-callsite records parsed. Outline callsites are
/// dense (one annotation per caller-method-and-helper pair); apps
/// can produce thousands. 1M is generous; cap-tripped logs WARN.
const MAX_OUTLINE_CALLSITES: usize = 1_000_000;

/// JSON-anchored ID matchers. Annotation comments parse as
/// `# {"id":"com.android.tools.r8.outline",...}`. Matching on the
/// raw substring would admit a sibling annotation whose payload
/// happens to contain the outline ID as a string value
/// (e.g., `{"id":"...synthesized","payload":"...outline"}`).
/// Anchoring on `"id":"<ID>"` requires the ID to occupy the JSON
/// id-field slot, not appear elsewhere.
const OUTLINE_ID_ANCHOR: &str = "\"id\":\"com.android.tools.r8.outline\"";
const OUTLINE_CALLSITE_ID_ANCHOR: &str = "\"id\":\"com.android.tools.r8.outlineCallsite\"";

/// R8's synthetic-class verbose-naming separator. When verbose
/// synthetic names are enabled (R8 internal flag
/// `enableVerboseSyntheticNames` — see programmatic Builder API
/// `setEnableVerboseSyntheticNames`), shareable synthetic kinds
/// embed the kind descriptor immediately after this infix:
/// `<context>$$ExternalSynthetic<KindDescriptor>$<id>`.
///
/// **Production R8 default is NOT verbose.** Per R8 source
/// `synthesis/SyntheticNaming.java:464-474`, when
/// `enableVerboseSyntheticNames == false` (the default), R8 emits
/// the **minimal** `<context>$<id>` shape using the bare inner-
/// class separator. The kind descriptor is stripped from the LHS
/// class name; the classifier below cannot recover the kind from
/// name alone and returns `SyntheticKind::Unknown` for these
/// minimal-shape synthetics. The Unknown bucket absorbing this
/// case is correct behaviour; per-kind reporting is structurally
/// limited to verbose-named builds.
///
/// **Verbose mode is internal-only.** R8 9.0.32 and 9.1.31 both
/// define the CLI constant `--verbose-synthetic-names` in
/// `BaseCompilerCommandParser.java` but the parser does NOT honour
/// it ("Unknown option" on attempted use). The flag is reachable
/// only programmatically via the Builder API, which means standard
/// release-channel APK builds — including reproducible F-Droid
/// builds and developer-published mappings like AnkiDroid's —
/// uniformly emit the minimal shape.
///
/// **Implication for the Arc 3 classifier.** The
/// `$$ExternalSynthetic` matching path is exercised in tests via
/// hand-crafted mapping inputs but is rarely hit on real R8
/// output. Tests that ASSERT specific kinds via class name will
/// pass under verbose mode and degrade gracefully to Unknown
/// under default minimal mode.
const EXTERNAL_SYNTHETIC_INFIX: &str = "$$ExternalSynthetic";

/// Legacy R8 pre-modern synthetic-outline class name. Before the
/// `$$ExternalSynthetic<Kind>` shape was introduced, R8 emitted
/// outline helpers as inner classes of a single
/// `GeneratedOutlineSupport` host. Mappings produced by old R8
/// versions still surface this name on the LHS.
const LEGACY_GENERATED_OUTLINE_SUPPORT: &str = "GeneratedOutlineSupport";

/// R8 `ENUM_UNBOXING_HELPER` synthetic-class infix. R8's enum-
/// unboxing transformation (per `synthesis/SyntheticNaming.java`,
/// kind `ENUM_UNBOXING_HELPER`) emits a per-host utility class
/// whose LHS class name is `<HostEnumType>$EnumUnboxingLocalUtility`
/// — e.g. `androidx.work.NetworkType$EnumUnboxingLocalUtility`.
/// The `$` here is the bare inner-class separator (not the `$$`
/// verbose-synthetic infix), so this matcher fires under both
/// minimal and verbose synthetic naming modes.
const ENUM_UNBOXING_LOCAL_UTILITY_SUFFIX: &str = "$EnumUnboxingLocalUtility";

/// SyntheticKind discriminator parsed from the original (LHS)
/// class name of a mapping.txt record. Verified against R8 source
/// `synthesis/SyntheticNaming.java`:
///
/// | Kind | Descriptor | Constructor |
/// |---|---|---|
/// | OUTLINE | `Outline` | `forSingleMethod("Outline")` |
/// | COVARIANT_OUTLINE | `CovariantOutline` | `forSingleMethod("CovariantOutline")` |
/// | API_MODEL_OUTLINE | `ApiModelOutline` | `forSingleMethodWithGlobalMerging("ApiModelOutline")` |
/// | API_MODEL_OUTLINE_WITHOUT_GLOBAL_MERGING | `ApiModelOutline` | `forSingleMethod("ApiModelOutline")` |
/// | NON_STARTUP_IN_STARTUP_OUTLINE | `NonStartupInStartupOutline` | `forSingleMethodWithGlobalMerging(...)` |
/// | BOTTOM_UP_OUTLINE | `BUOutline` | `forSingleMethodWithGlobalMerging("BUOutline")` |
/// | OBJECT_CLONE_OUTLINE | `ObjectCloneOutline` | `forSingleMethod("ObjectCloneOutline")` |
///
/// API_MODEL_OUTLINE and API_MODEL_OUTLINE_WITHOUT_GLOBAL_MERGING
/// share descriptor `ApiModelOutline` and are descriptor-
/// indistinguishable from the LHS class name alone — they collapse
/// into a single `ApiModelOutline` bucket. Distinguishing them
/// would require parsing R8's compiler-info comment which is not
/// guaranteed to survive across R8 versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyntheticKind {
    Outline,
    CovariantOutline,
    /// Combined bucket for API_MODEL_OUTLINE and
    /// API_MODEL_OUTLINE_WITHOUT_GLOBAL_MERGING. See enum docstring.
    ApiModelOutline,
    NonStartupInStartupOutline,
    BottomUpOutline,
    ObjectCloneOutline,
    /// Pre-modern R8 outline emit shape (`GeneratedOutlineSupport`).
    LegacyGeneratedOutlineSupport,
    /// R8's `ENUM_UNBOXING_HELPER` synthetic kind (per
    /// `synthesis/SyntheticNaming.java`). Anchored on the
    /// `$EnumUnboxingLocalUtility` suffix on the last segment of
    /// the LHS class name; the host segment is the original boxed
    /// enum type being unboxed (e.g. `androidx.work.NetworkType`
    /// in `androidx.work.NetworkType$EnumUnboxingLocalUtility`).
    ///
    /// **This is NOT an outline-emitting kind.** Enum unboxing is a
    /// separate R8 transformation that converts boxed enum constants
    /// into int operations; the utility class hosts the dispatch
    /// helpers used by the unboxed call sites. The variant is added
    /// because the F-Droid sweep surfaced this emit shape at scale
    /// (n=1203, 70+ hits on a single `androidx.work` host alone).
    /// Classifying it as [`Self::Unknown`] would conflate a known R8
    /// transformation with truly unrecognised classes; classifying
    /// it as [`Self::OutlineKindUnknown`] would falsely claim the
    /// class hosts outlined methods. Its own bucket keeps the kind
    /// axis honest.
    EnumUnboxing,
    /// Joint-signal bucket for R8 production-default emit. The
    /// outline body annotation (`com.android.tools.r8.outline`) IS
    /// authoritative for "this method is outlined", but the LHS
    /// class name uses R8's minimal `<context>$<id>` shape (the
    /// production default per [[#external_synthetic_infix]]) which
    /// strips the kind discriminator from the name. The kind is
    /// known to be ONE OF the seven outline-emitting kinds — we
    /// just can't tell which from the mapping alone.
    ///
    /// This is structurally different from [`Self::Unknown`]:
    /// `OutlineKindUnknown` is "yes-it-is-outlined, kind-ambiguous";
    /// `Unknown` is "we have no signal at all" (developer code,
    /// new R8 SyntheticKind we don't recognise, crafted input).
    /// Both are sound — neither overclaims the kind axis — but the
    /// split lets analyst output distinguish the two cases.
    OutlineKindUnknown,
    /// Mapping LHS does not match any known outline-emitting
    /// SyntheticKind AND no outline annotation associates this
    /// class with the outliner. The annotation is still recorded
    /// (the `com.android.tools.r8.outline` id is authoritative),
    /// but the kind axis cannot be filled in from this parser's
    /// signals. New SyntheticKinds added in future R8 versions
    /// also land here when their naming doesn't match any known
    /// pattern.
    Unknown,
}

impl SyntheticKind {
    /// Stable label for harness output (`OUTLINE`, `BOTTOM_UP_OUTLINE`,
    /// etc.). Mirrors the R8 source enum spelling so analysts can grep
    /// the recogniser output for the exact SyntheticNaming.java symbol.
    pub fn label(self) -> &'static str {
        match self {
            SyntheticKind::Outline => "OUTLINE",
            SyntheticKind::CovariantOutline => "COVARIANT_OUTLINE",
            SyntheticKind::ApiModelOutline => "API_MODEL_OUTLINE",
            SyntheticKind::NonStartupInStartupOutline => "NON_STARTUP_IN_STARTUP_OUTLINE",
            SyntheticKind::BottomUpOutline => "BOTTOM_UP_OUTLINE",
            SyntheticKind::ObjectCloneOutline => "OBJECT_CLONE_OUTLINE",
            SyntheticKind::LegacyGeneratedOutlineSupport => "LEGACY_GENERATED_OUTLINE_SUPPORT",
            SyntheticKind::EnumUnboxing => "ENUM_UNBOXING",
            SyntheticKind::OutlineKindUnknown => "OUTLINE_KIND_UNKNOWN",
            SyntheticKind::Unknown => "UNKNOWN",
        }
    }

    /// Ordering used for the harness breakdown table. Modern outline-
    /// emitting kinds first (in R8 source declaration order), legacy +
    /// non-outline known kinds + joint-signal + unknown last.
    pub fn report_order() -> &'static [SyntheticKind] {
        &[
            SyntheticKind::Outline,
            SyntheticKind::CovariantOutline,
            SyntheticKind::ApiModelOutline,
            SyntheticKind::NonStartupInStartupOutline,
            SyntheticKind::BottomUpOutline,
            SyntheticKind::ObjectCloneOutline,
            SyntheticKind::LegacyGeneratedOutlineSupport,
            SyntheticKind::EnumUnboxing,
            SyntheticKind::OutlineKindUnknown,
            SyntheticKind::Unknown,
        ]
    }
}

/// Classify a class name into a SyntheticKind by examining its
/// substring shape. Returns [`SyntheticKind::Unknown`] for class names
/// that don't carry a recognised infix/suffix — most developer
/// classes fall here. The function does not assume input validation;
/// callers MAY pass any string. Legacy `GeneratedOutlineSupport` and
/// `EnumUnboxingLocalUtility` are checked first because both predate
/// or sit outside the `$$ExternalSynthetic` infix shape.
///
/// The legacy check anchors on the trailing dot-separated segment
/// (with optional `$`-suffix for inner classes) so that developer
/// classes embedding the substring (e.g.
/// `com.foo.MyGeneratedOutlineSupportWrapper`) are not mis-bucketed.
/// The enum-unboxing check anchors on `ends_with` of the trailing
/// segment so an embed inside a longer name (e.g.
/// `com.foo.MyEnumUnboxingLocalUtilityWrapper`) is not bucketed.
///
/// ## Trust model
///
/// This function checks for substring patterns. The PATTERNS themselves
/// (`$EnumUnboxingLocalUtility`, `GeneratedOutlineSupport`,
/// `$$ExternalSynthetic<kind>`) are R8-emitted artifacts; an attacker
/// without control of the R8 toolchain cannot forge them in a way that
/// also satisfies the bytecode-level outline structural invariants and
/// the R8 outline annotation. However, an attacker who controls the
/// class name string ALONE (e.g., compiling DEX directly via smali or
/// raw bytecode toolchains) CAN embed these patterns. Use this function
/// as ground-truth ONLY in mapping-paired contexts where the input is
/// the `mapping.txt` LHS (original class name) produced by R8 itself.
/// In mapping-less contexts the function is a hint, not a verdict —
/// pair with the `com.android.tools.r8.outline` annotation when
/// available, or fall through to structural invariants (I4–I13) for
/// the recogniser's primary judgement.
pub fn classify_synthetic_kind(original_class_name: &str) -> SyntheticKind {
    let last_segment = original_class_name
        .rsplit('.')
        .next()
        .unwrap_or(original_class_name);
    if last_segment.ends_with(ENUM_UNBOXING_LOCAL_UTILITY_SUFFIX) {
        return SyntheticKind::EnumUnboxing;
    }
    if let Some(suffix) = last_segment.strip_prefix(LEGACY_GENERATED_OUTLINE_SUPPORT) {
        if suffix.is_empty() || suffix.starts_with('$') {
            return SyntheticKind::LegacyGeneratedOutlineSupport;
        }
    }
    let Some(infix_pos) = original_class_name.find(EXTERNAL_SYNTHETIC_INFIX) else {
        return SyntheticKind::Unknown;
    };
    let after = &original_class_name[infix_pos + EXTERNAL_SYNTHETIC_INFIX.len()..];
    // The descriptor is the longest leading ASCII-alphabetic run
    // after the infix. R8 appends `<id>` numerically (sometimes with
    // a `$` separator, sometimes without) so the alphabetic prefix is
    // the kind descriptor. Anchoring on alphabetic-only rejects
    // empty-descriptor crafted input (`$$ExternalSynthetic$0`).
    let descriptor_len = after
        .bytes()
        .take_while(|b| b.is_ascii_alphabetic())
        .count();
    let descriptor = &after[..descriptor_len];
    match descriptor {
        "Outline" => SyntheticKind::Outline,
        "CovariantOutline" => SyntheticKind::CovariantOutline,
        "ApiModelOutline" => SyntheticKind::ApiModelOutline,
        "NonStartupInStartupOutline" => SyntheticKind::NonStartupInStartupOutline,
        "BUOutline" => SyntheticKind::BottomUpOutline,
        "ObjectCloneOutline" => SyntheticKind::ObjectCloneOutline,
        _ => SyntheticKind::Unknown,
    }
}

/// Set of outlined methods + callers, parsed from a mapping.txt body.
#[derive(Debug, Default)]
pub struct OutlineSet {
    /// (obfuscated_class, obfuscated_method) tuples that carry the
    /// `com.android.tools.r8.outline` annotation. These are
    /// outlined-method BODIES.
    outlined_methods: BTreeSet<(String, String)>,
    /// For each outlined method, the SyntheticKind inferred from the
    /// LHS (original) class name of the class record that contained
    /// it. [`SyntheticKind::Unknown`] for class names that don't
    /// match any known outline-emitting kind — the annotation is
    /// still authoritative ("this IS outlined") but the kind axis is
    /// unfilled.
    outlined_method_kinds: BTreeMap<(String, String), SyntheticKind>,
    /// (obfuscated_class, obfuscated_method) tuples that carry the
    /// `com.android.tools.r8.outlineCallsite` annotation. These are
    /// CALLERS of outlined helpers.
    outline_callsites: BTreeSet<(String, String)>,
    /// For each outline-callsite, the set of helper-method
    /// references the callsite invokes (the JSON `"outline"` field).
    /// Multiset rather than single value because a single
    /// caller-method can invoke multiple distinct outlined helpers.
    callsite_helper_refs: BTreeMap<(String, String), BTreeSet<String>>,
    /// Outlined-method tuples that appeared in the input more than
    /// once. Non-empty under malformed or crafted input.
    duplicate_outline_methods: BTreeSet<(String, String)>,
    /// Outline-callsite tuples whose annotation included a
    /// `"outline"` field whose value differs across appearances —
    /// records the conflict. R8 emits one annotation per
    /// (caller-method, helper) tuple, so multiple distinct helpers
    /// per caller-method are legitimate.
    callsite_helper_conflicts: BTreeSet<(String, String)>,
    /// True when `MAX_OUTLINE_CALLSITES` was tripped.
    callsite_cap_tripped: bool,
    /// Count of outline annotations dropped because their distance
    /// from the most recent anchor method exceeded
    /// [`OUTLINE_PROXIMITY_LINES`].
    outline_proximity_dropped: usize,
    /// True when `MAX_OUTLINE_METHODS` was tripped (partial parse).
    cap_tripped: bool,
}

impl OutlineSet {
    /// Parse outline + outlineCallsite annotations from a mapping.txt body.
    pub fn parse(text: &str) -> Self {
        let mut outlined_methods: BTreeSet<(String, String)> = BTreeSet::new();
        let mut outlined_method_kinds: BTreeMap<(String, String), SyntheticKind> = BTreeMap::new();
        let mut outline_callsites: BTreeSet<(String, String)> = BTreeSet::new();
        let mut callsite_helper_refs: BTreeMap<(String, String), BTreeSet<String>> =
            BTreeMap::new();
        let mut duplicate_outline_methods: BTreeSet<(String, String)> = BTreeSet::new();
        let mut callsite_helper_conflicts: BTreeSet<(String, String)> = BTreeSet::new();
        let mut seen_outline_methods: BTreeSet<(String, String)> = BTreeSet::new();
        let mut outline_proximity_dropped = 0usize;
        let mut cap_tripped = false;
        let mut callsite_cap_tripped = false;

        let mut current_class: Option<String> = None;
        let mut current_class_kind: SyntheticKind = SyntheticKind::Unknown;
        let mut current_method: Option<String> = None;
        let mut anchor_line: Option<usize> = None;

        for (line_no, raw_line) in text.lines().enumerate() {
            let trimmed = raw_line.trim_start();

            // Class records are unindented and end with `:`.
            if let Some((orig, obf)) = parse_class_record(raw_line) {
                current_class_kind = classify_synthetic_kind(&orig);
                current_class = Some(obf);
                current_method = None;
                anchor_line = Some(line_no);
                continue;
            }

            // Annotation comments start with `#`. Check them before
            // attempting method-record parsing — annotation lines
            // also begin with whitespace but the `#` prefix makes
            // them unambiguous.
            if trimmed.starts_with('#') {
                // Anchor on the JSON id-field shape rather than a raw
                // substring of the ID. Without anchoring, an unrelated
                // annotation whose payload contains the ID string in
                // any field would match.
                let is_outline = trimmed.contains(OUTLINE_ID_ANCHOR);
                let is_callsite = trimmed.contains(OUTLINE_CALLSITE_ID_ANCHOR);
                if !is_outline && !is_callsite {
                    continue;
                }
                if is_callsite {
                    if let (Some(c), Some(m), Some(anchor)) =
                        (current_class.as_ref(), current_method.as_ref(), anchor_line)
                    {
                        if line_no.saturating_sub(anchor) <= OUTLINE_PROXIMITY_LINES {
                            if outline_callsites.len() < MAX_OUTLINE_CALLSITES {
                                let key = (c.clone(), m.clone());
                                outline_callsites.insert(key.clone());
                                if let Some(helper) = parse_outline_field(trimmed) {
                                    let entry = callsite_helper_refs
                                        .entry(key.clone())
                                        .or_default();
                                    if !entry.is_empty() && !entry.contains(&helper) {
                                        callsite_helper_conflicts.insert(key);
                                    }
                                    entry.insert(helper);
                                }
                            } else if !callsite_cap_tripped {
                                callsite_cap_tripped = true;
                                eprintln!(
                                    "WARN: OutlineSet::parse hit MAX_OUTLINE_CALLSITES={MAX_OUTLINE_CALLSITES}; further outline-callsite records ignored. Mapping is unusually large or crafted."
                                );
                            }
                        } else {
                            outline_proximity_dropped =
                                outline_proximity_dropped.saturating_add(1);
                        }
                    }
                    continue;
                }
                // outline (body) annotation.
                if let (Some(c), Some(m), Some(anchor)) =
                    (current_class.as_ref(), current_method.as_ref(), anchor_line)
                {
                    if line_no.saturating_sub(anchor) <= OUTLINE_PROXIMITY_LINES {
                        let key = (c.clone(), m.clone());
                        if outlined_methods.len() < MAX_OUTLINE_METHODS {
                            if !seen_outline_methods.insert(key.clone()) {
                                duplicate_outline_methods.insert(key.clone());
                            }
                            // Joint-signal upgrade. If the class
                            // name didn't yield a kind (Unknown from
                            // classify_synthetic_kind), but the
                            // outline-body annotation IS present
                            // here, attribute as OutlineKindUnknown
                            // — verified outline, kind ambiguous due
                            // to R8's production-default minimal
                            // naming. See SyntheticKind variant
                            // docstrings for the split rationale.
                            let kind_to_record = if matches!(
                                current_class_kind,
                                SyntheticKind::Unknown
                            ) {
                                SyntheticKind::OutlineKindUnknown
                            } else {
                                current_class_kind
                            };
                            outlined_method_kinds
                                .entry(key.clone())
                                .or_insert(kind_to_record);
                            outlined_methods.insert(key);
                        } else if !cap_tripped {
                            cap_tripped = true;
                            eprintln!(
                                "WARN: OutlineSet::parse hit MAX_OUTLINE_METHODS={MAX_OUTLINE_METHODS}; further outline records ignored. Mapping is unusually large or crafted."
                            );
                        }
                    } else {
                        outline_proximity_dropped =
                            outline_proximity_dropped.saturating_add(1);
                    }
                }
                continue;
            }

            // Method records are indented and contain ` -> `.
            // Update cursors when we see one.
            if let Some(obf_method) = parse_method_record(raw_line) {
                current_method = Some(obf_method);
                anchor_line = Some(line_no);
            }
        }

        Self {
            outlined_methods,
            outlined_method_kinds,
            outline_callsites,
            callsite_helper_refs,
            duplicate_outline_methods,
            callsite_helper_conflicts,
            outline_proximity_dropped,
            cap_tripped,
            callsite_cap_tripped,
        }
    }

    /// Read + parse a mapping file from disk with adversarial-input
    /// hardening. Rejects symlinks (CI-poisoning vector) and files
    /// larger than [`MAX_MAPPING_BYTES`] (resource exhaustion).
    /// Hardlink swap is NOT defended — `fs::symlink_metadata` only
    /// catches symlinks; treating hardlinks defensively requires
    /// inode-ownership checks outside this parser's scope.
    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        let meta = std::fs::symlink_metadata(path)?;
        if meta.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("symlink rejected: {}", path.display()),
            ));
        }
        if meta.len() > MAX_MAPPING_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "{} bytes exceeds {MAX_MAPPING_BYTES}-byte cap",
                    meta.len()
                ),
            ));
        }
        let text = std::fs::read_to_string(path)?;
        Ok(Self::parse(&text))
    }

    /// True if (obfuscated_class, obfuscated_method) is in the
    /// outline-body annotation set.
    pub fn is_outlined(&self, class: &str, method: &str) -> bool {
        self.outlined_methods
            .contains(&(class.to_string(), method.to_string()))
    }

    /// SyntheticKind inferred for the named outlined method from its
    /// containing class's LHS (original) name in mapping.txt. Returns
    /// `None` if the (class, method) tuple is not in the outline-body
    /// annotation set. Returns `Some(SyntheticKind::Unknown)` if the
    /// tuple IS annotated but the LHS class name didn't match any
    /// recognised outline-emitting kind (legacy R8, post-modern R8
    /// with new kinds, or crafted input).
    pub fn outlined_kind(&self, class: &str, method: &str) -> Option<SyntheticKind> {
        self.outlined_method_kinds
            .get(&(class.to_string(), method.to_string()))
            .copied()
    }

    /// Per-SyntheticKind annotation counts. Every outlined-method
    /// tuple is attributed to exactly one kind (possibly
    /// [`SyntheticKind::Unknown`]); summing the map yields
    /// [`Self::outlined_count`].
    pub fn outlined_kind_counts(&self) -> BTreeMap<SyntheticKind, usize> {
        let mut counts: BTreeMap<SyntheticKind, usize> = BTreeMap::new();
        for kind in self.outlined_method_kinds.values() {
            let slot = counts.entry(*kind).or_insert(0);
            *slot = slot.saturating_add(1);
        }
        counts
    }

    /// Per-kind matched/annotated breakdown for the harness output.
    /// Given the set of (class, method) tuples for which a recogniser
    /// marker fired, returns one row per [`SyntheticKind::report_order`]
    /// entry: `(kind, annotated, matched)`. `annotated` is the count
    /// of outline-set tuples bucketed to this kind; `matched` is the
    /// subset whose tuple is in `fired`. Rows are emitted in stable
    /// report order so harness output is diff-friendly across runs.
    pub fn per_kind_match_report(
        &self,
        fired: &BTreeSet<(String, String)>,
    ) -> Vec<(SyntheticKind, usize, usize)> {
        let counts = self.outlined_kind_counts();
        let mut matched: BTreeMap<SyntheticKind, usize> = BTreeMap::new();
        for (tuple, kind) in &self.outlined_method_kinds {
            if fired.contains(tuple) {
                let slot = matched.entry(*kind).or_insert(0);
                *slot = slot.saturating_add(1);
            }
        }
        SyntheticKind::report_order()
            .iter()
            .map(|kind| {
                (
                    *kind,
                    counts.get(kind).copied().unwrap_or(0),
                    matched.get(kind).copied().unwrap_or(0),
                )
            })
            .collect()
    }

    /// True if (obfuscated_class, obfuscated_method) is in the
    /// outline-callsite annotation set.
    pub fn is_outline_callsite(&self, class: &str, method: &str) -> bool {
        self.outline_callsites
            .contains(&(class.to_string(), method.to_string()))
    }

    /// The set of helper-method references the named callsite
    /// invokes, if any. A single caller-method may invoke multiple
    /// outlined helpers (R8 emits one annotation per
    /// caller-method-and-helper tuple).
    pub fn callsite_helpers(&self, class: &str, method: &str) -> Option<&BTreeSet<String>> {
        self.callsite_helper_refs
            .get(&(class.to_string(), method.to_string()))
    }

    pub fn outlined_count(&self) -> usize {
        self.outlined_methods.len()
    }

    pub fn outline_callsite_count(&self) -> usize {
        self.outline_callsites.len()
    }

    pub fn duplicate_outline_methods(&self) -> &BTreeSet<(String, String)> {
        &self.duplicate_outline_methods
    }

    /// Callsite tuples whose annotation included a `"outline"`
    /// field whose value differs across appearances. These are
    /// callers that invoke multiple distinct outlined helpers.
    pub fn callsite_helper_conflicts(&self) -> &BTreeSet<(String, String)> {
        &self.callsite_helper_conflicts
    }

    pub fn outline_proximity_dropped(&self) -> usize {
        self.outline_proximity_dropped
    }

    pub fn cap_tripped(&self) -> bool {
        self.cap_tripped
    }

    pub fn callsite_cap_tripped(&self) -> bool {
        self.callsite_cap_tripped
    }

    /// Iterator over the outlined-method set. (class, method).
    pub fn outlined_methods(&self) -> impl Iterator<Item = (&str, &str)> {
        self.outlined_methods
            .iter()
            .map(|(c, m)| (c.as_str(), m.as_str()))
    }
}

/// Recognise a class record line: unindented `original.name -> obf:`.
/// Returns `(original, obfuscated)`. Returns `None` for any line that
/// doesn't match the strict shape (indented, missing arrow, missing
/// trailing `:`, empty halves).
///
/// The original name is needed for SyntheticKind classification — the
/// LHS preserves R8's pre-minification `$$ExternalSynthetic<Kind>`
/// infix. See [`classify_synthetic_kind`].
fn parse_class_record(line: &str) -> Option<(String, String)> {
    if line.is_empty() {
        return None;
    }
    // Unindented: first byte is not whitespace.
    let first = line.as_bytes()[0];
    if first == b' ' || first == b'\t' {
        return None;
    }
    // Must end with `:`.
    let stripped = line.strip_suffix(':')?;
    let (orig, obf) = stripped.split_once(" -> ")?;
    if orig.is_empty() || obf.is_empty() {
        return None;
    }
    Some((orig.to_string(), obf.to_string()))
}

/// Recognise a method record line and return the obfuscated method
/// name. Method records are indented, begin with a
/// `<digits>:<digits>:` line-number range (R8 always emits this for
/// non-inlined methods), and contain ` -> `.
///
/// Examples:
///   `    1:15:int outlineCaller(int):98:98 -> s`
///   `    1:15:int outline() -> a`
///
/// The line-number prefix gate rejects indented comment / wrapped
/// text lines that happen to contain ` -> <ident>` (e.g.,
/// `    SomeAnnotation -> RetentionPolicy.RUNTIME` in a class
/// declaration's annotation list).
fn parse_method_record(line: &str) -> Option<String> {
    let trimmed = line.strip_prefix(' ').or_else(|| line.strip_prefix('\t'))?;
    let trimmed = trimmed.trim_start();
    // Require `<digits>:<digits>:` line-number prefix.
    let after_first_num = drop_leading_digits(trimmed)?;
    let after_colon1 = after_first_num.strip_prefix(':')?;
    let after_second_num = drop_leading_digits(after_colon1)?;
    if !after_second_num.starts_with(':') {
        return None;
    }
    // Now safely look for the obfuscated name.
    let (_, obf) = line.rsplit_once(" -> ")?;
    let obf = obf.trim();
    if obf.is_empty() {
        return None;
    }
    // Obfuscated method name must be a plain identifier — no dots,
    // no whitespace, no quotes / braces.
    if obf
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '<' || c == '>'))
    {
        return None;
    }
    Some(obf.to_string())
}

/// Consume the leading run of ASCII digits from `s`. Returns the
/// remainder, or `None` if there were no leading digits.
fn drop_leading_digits(s: &str) -> Option<&str> {
    let mut idx = 0;
    for b in s.bytes() {
        if b.is_ascii_digit() {
            idx += 1;
        } else {
            break;
        }
    }
    if idx == 0 {
        None
    } else {
        Some(&s[idx..])
    }
}

/// Extract the `"outline":"<value>"` field from an outlineCallsite
/// JSON annotation. Returns `None` if absent or malformed.
fn parse_outline_field(annotation: &str) -> Option<String> {
    let needle = "\"outline\":\"";
    let start = annotation.find(needle)?;
    let after = &annotation[start + needle.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANONICAL_OUTLINE_BLOCK: &str = "\
# compiler: R8
# compiler_version: 9.0.32
# {\"id\":\"com.android.tools.r8.mapping\",\"version\":\"2.2\"}
outline.Class -> a:
    1:2:int outline() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
some.Class -> b:
    1:1:void original.Method.foo():42:42 -> s
    4:4:int outlineCaller(int):98:98 -> s
    5:5:int outlineCaller(int):100:100 -> s
    # {\"id\":\"com.android.tools.r8.outlineCallsite\",\"positions\":{\"1\":4,\"2\":5},\"outline\":\"La;a()I\"}
";

    #[test]
    fn canonical_outline_block_parses() {
        let s = OutlineSet::parse(CANONICAL_OUTLINE_BLOCK);
        assert!(s.is_outlined("a", "a"));
        assert_eq!(s.outlined_count(), 1);
        assert!(s.is_outline_callsite("b", "s"));
        assert_eq!(s.outline_callsite_count(), 1);
        let helpers = s
            .callsite_helpers("b", "s")
            .expect("callsite helpers recorded");
        assert!(helpers.contains("La;a()I"));
        assert_eq!(s.outline_proximity_dropped(), 0);
        assert!(s.duplicate_outline_methods().is_empty());
        assert!(s.callsite_helper_conflicts().is_empty());
        assert!(!s.cap_tripped());
        assert!(!s.callsite_cap_tripped());
    }

    #[test]
    fn callsite_with_multiple_distinct_helpers_records_conflict() {
        // Single caller-method invokes two distinct outlined helpers.
        // R8 emits one annotation per (caller, helper); we record
        // both helpers and flag the conflict.
        let body = "\
some.Class -> b:
    1:1:int caller():42:42 -> s
    # {\"id\":\"com.android.tools.r8.outlineCallsite\",\"positions\":{\"1\":4},\"outline\":\"La;a()I\"}
    # {\"id\":\"com.android.tools.r8.outlineCallsite\",\"positions\":{\"2\":5},\"outline\":\"Lb;b()V\"}
";
        let s = OutlineSet::parse(body);
        let helpers = s.callsite_helpers("b", "s").expect("recorded");
        assert_eq!(helpers.len(), 2);
        assert!(helpers.contains("La;a()I"));
        assert!(helpers.contains("Lb;b()V"));
        assert!(s.callsite_helper_conflicts().contains(&("b".into(), "s".into())));
    }

    #[test]
    fn id_substring_in_payload_does_not_match() {
        // Synthesized annotation whose payload string contains the
        // outline ID as a value (not as the "id" field). Must not
        // be counted as an outline annotation.
        let body = "\
some.Class -> a:
    1:1:int foo() -> b
    # {\"id\":\"com.android.tools.r8.synthesized\",\"note\":\"com.android.tools.r8.outline string in a value field\"}
";
        let s = OutlineSet::parse(body);
        assert_eq!(s.outlined_count(), 0);
        assert_eq!(s.outline_callsite_count(), 0);
    }

    #[test]
    fn comment_with_arrow_does_not_become_method_record() {
        // An indented comment with ` -> ` should NOT update
        // current_method. R8 method records always carry the
        // <digits>:<digits>: line-number prefix.
        let body = "\
some.Class -> a:
    SomeAnnotation -> RetentionPolicy.RUNTIME
    # {\"id\":\"com.android.tools.r8.outline\"}
";
        let s = OutlineSet::parse(body);
        // current_method never got set (no real method record), so
        // the outline annotation should not attach to anything.
        assert_eq!(s.outlined_count(), 0);
    }

    #[test]
    fn outline_callsite_not_confused_with_outline_body() {
        // outlineCallsite is a substring superset of outline; ensure
        // the parser counts each separately.
        let s = OutlineSet::parse(CANONICAL_OUTLINE_BLOCK);
        assert_eq!(s.outlined_count(), 1);
        assert_eq!(s.outline_callsite_count(), 1);
    }

    #[test]
    fn annotation_beyond_proximity_does_not_count() {
        // 17-line gap between method anchor and outline annotation.
        let mut body = String::from("outline.Class -> a:\n    1:2:int outline() -> a\n");
        for _ in 0..17 {
            body.push_str("# filler line\n");
        }
        body.push_str("    # {\"id\":\"com.android.tools.r8.outline\"}\n");
        let s = OutlineSet::parse(&body);
        assert_eq!(s.outlined_count(), 0);
        assert_eq!(s.outline_proximity_dropped(), 1);
    }

    #[test]
    fn unrelated_annotation_ids_ignored() {
        // synthesized and residualsignature annotations don't fire
        // this parser. Only outline + outlineCallsite count.
        let body = "\
some.Class -> a:
    1:1:int foo() -> b
    # {\"id\":\"com.android.tools.r8.synthesized\"}
    1:1:int bar() -> c
    # {\"id\":\"com.android.tools.r8.residualsignature\",\"signature\":\"(II)I\"}
";
        let s = OutlineSet::parse(body);
        assert_eq!(s.outlined_count(), 0);
        assert_eq!(s.outline_callsite_count(), 0);
    }

    #[test]
    fn empty_input_ok() {
        let s = OutlineSet::parse("");
        assert_eq!(s.outlined_count(), 0);
        assert_eq!(s.outline_callsite_count(), 0);
        assert!(!s.cap_tripped());
    }

    #[test]
    fn duplicate_outline_method_detected() {
        // Same obfuscated (class, method) appears with the outline
        // annotation twice. Real R8 output shouldn't do this; a
        // crafted mapping might.
        let body = "\
first.Class -> a:
    1:1:int outline() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
second.Class -> a:
    1:1:int outline() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
";
        let s = OutlineSet::parse(body);
        assert!(!s.duplicate_outline_methods().is_empty());
    }

    #[test]
    fn outline_field_extraction() {
        let blob = "# {\"id\":\"com.android.tools.r8.outlineCallsite\",\"positions\":{\"1\":4},\"outline\":\"Lhf1;s(II)V\"}";
        assert_eq!(parse_outline_field(blob), Some("Lhf1;s(II)V".to_string()));
    }

    #[test]
    fn outline_field_missing_returns_none() {
        let blob = "# {\"id\":\"com.android.tools.r8.outline\"}";
        assert_eq!(parse_outline_field(blob), None);
    }

    #[test]
    fn class_record_parser_strict() {
        assert_eq!(
            parse_class_record("foo.Bar -> a:"),
            Some(("foo.Bar".to_string(), "a".to_string())),
        );
        // Indented (method record, not class).
        assert!(parse_class_record("    foo.Bar -> a:").is_none());
        // Missing colon.
        assert!(parse_class_record("foo.Bar -> a").is_none());
        // Missing arrow.
        assert!(parse_class_record("foo.Bar a:").is_none());
        // Empty obf.
        assert!(parse_class_record("foo.Bar -> :").is_none());
    }

    #[test]
    fn method_record_parser_strict() {
        assert_eq!(
            parse_method_record("    1:15:int outline() -> a").as_deref(),
            Some("a")
        );
        // Unindented (class record, not method).
        assert!(parse_method_record("foo.Bar -> a").is_none());
        // No arrow.
        assert!(parse_method_record("    1:15:int outline()").is_none());
        // Whitespace in obf rejected.
        assert!(parse_method_record("    foo -> a b").is_none());
    }

    #[test]
    fn marker_inside_method_signature_does_not_count() {
        // R8 method records have ` -> ` separators. A class whose
        // ORIGINAL Java signature literally contains the outline
        // annotation text would still not satisfy the `# ...`
        // comment prefix check.
        let body = "\
some.Class -> a:
    1:1:String foo(String contains_com_android_tools_r8_outline) -> b
";
        let s = OutlineSet::parse(body);
        assert_eq!(s.outlined_count(), 0);
    }

    #[test]
    fn synthetic_kind_classifier_recognises_each_descriptor() {
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSyntheticOutline$0"),
            SyntheticKind::Outline,
        );
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSyntheticCovariantOutline$0"),
            SyntheticKind::CovariantOutline,
        );
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSyntheticApiModelOutline$0"),
            SyntheticKind::ApiModelOutline,
        );
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSyntheticNonStartupInStartupOutline$0"),
            SyntheticKind::NonStartupInStartupOutline,
        );
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSyntheticBUOutline$0"),
            SyntheticKind::BottomUpOutline,
        );
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSyntheticObjectCloneOutline$0"),
            SyntheticKind::ObjectCloneOutline,
        );
    }

    #[test]
    fn synthetic_kind_classifier_recognises_no_dollar_id_separator() {
        // R8's older builds and some shareable-merge cases emit the
        // numeric id directly after the descriptor with no `$`
        // separator. The classifier's alphabetic-only descriptor scan
        // must still recover the kind.
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSyntheticOutline0"),
            SyntheticKind::Outline,
        );
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSyntheticBUOutline42"),
            SyntheticKind::BottomUpOutline,
        );
    }

    #[test]
    fn synthetic_kind_classifier_handles_legacy() {
        // Pre-modern R8 emit shape — GeneratedOutlineSupport host
        // class. Classifier matches the bare class name and its
        // `$`-suffixed inner classes; both forms appear in legacy
        // mappings.
        assert_eq!(
            classify_synthetic_kind("com.foo.GeneratedOutlineSupport"),
            SyntheticKind::LegacyGeneratedOutlineSupport,
        );
        assert_eq!(
            classify_synthetic_kind("GeneratedOutlineSupport$inner"),
            SyntheticKind::LegacyGeneratedOutlineSupport,
        );
        assert_eq!(
            classify_synthetic_kind("GeneratedOutlineSupport$1"),
            SyntheticKind::LegacyGeneratedOutlineSupport,
        );
    }

    #[test]
    fn synthetic_kind_classifier_legacy_substring_not_misclassified() {
        // Developer class whose name embeds `GeneratedOutlineSupport`
        // as a substring (e.g. a wrapper) must not be bucketed as
        // legacy. Anchoring on the trailing dot-segment + optional
        // $-suffix prevents the substring leak.
        assert_eq!(
            classify_synthetic_kind("com.foo.MyGeneratedOutlineSupportWrapper"),
            SyntheticKind::Unknown,
        );
        assert_eq!(
            classify_synthetic_kind("com.foo.GeneratedOutlineSupportHelper"),
            SyntheticKind::Unknown,
        );
        // Package-only embed: still Unknown — the legacy host class
        // identifies by the LAST segment, not anywhere in the path.
        assert_eq!(
            classify_synthetic_kind("com.GeneratedOutlineSupport.RealClass"),
            SyntheticKind::Unknown,
        );
    }

    #[test]
    fn synthetic_kind_classifier_unknown_for_developer_code() {
        // Normal class names without the synthetic infix.
        assert_eq!(
            classify_synthetic_kind("com.example.MyActivity"),
            SyntheticKind::Unknown,
        );
        assert_eq!(classify_synthetic_kind(""), SyntheticKind::Unknown);
    }

    #[test]
    fn synthetic_kind_classifier_unknown_descriptor() {
        // ExternalSynthetic infix with a descriptor we don't
        // recognise. Could be a new R8 SyntheticKind, a D8 desugar
        // kind that shares the prefix, or crafted input. Falls
        // through to Unknown rather than silently mis-classifying.
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSyntheticLambda$0"),
            SyntheticKind::Unknown,
        );
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSyntheticOutlineX$0"),
            SyntheticKind::Unknown,
        );
    }

    #[test]
    fn synthetic_kind_classifier_empty_descriptor_rejected() {
        // Crafted: $$ExternalSynthetic immediately followed by a
        // non-alphabetic byte. The alphabetic-only scan yields an
        // empty descriptor; classifier returns Unknown rather than
        // accidentally matching the empty-string arm.
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSynthetic$0"),
            SyntheticKind::Unknown,
        );
        assert_eq!(
            classify_synthetic_kind("foo.Bar$$ExternalSynthetic"),
            SyntheticKind::Unknown,
        );
    }

    #[test]
    fn mapping_attributes_outline_to_kind() {
        // End-to-end: outline annotation on a $$ExternalSyntheticOutline
        // class should attribute the (class, method) tuple to
        // SyntheticKind::Outline.
        let body = "\
com.example.Foo$$ExternalSyntheticOutline$0 -> aa:
    1:2:int outline() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
com.example.Foo$$ExternalSyntheticBUOutline$0 -> bb:
    1:2:int outline() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
com.example.Real -> rr:
    1:1:void notOutlined() -> n
";
        let s = OutlineSet::parse(body);
        assert_eq!(s.outlined_count(), 2);
        assert_eq!(s.outlined_kind("aa", "a"), Some(SyntheticKind::Outline));
        assert_eq!(s.outlined_kind("bb", "a"), Some(SyntheticKind::BottomUpOutline));
        // Real class not annotated → no kind entry.
        assert_eq!(s.outlined_kind("rr", "n"), None);
    }

    #[test]
    fn mapping_legacy_outline_annotated_class_gets_legacy_bucket() {
        // Legacy R8 emitted outline methods inside the
        // GeneratedOutlineSupport host class. The outline annotation
        // still attaches; the kind bucket is LegacyGeneratedOutlineSupport.
        let body = "\
com.example.GeneratedOutlineSupport -> g:
    1:1:int outline() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
";
        let s = OutlineSet::parse(body);
        assert_eq!(
            s.outlined_kind("g", "a"),
            Some(SyntheticKind::LegacyGeneratedOutlineSupport),
        );
    }

    #[test]
    fn mapping_unknown_class_annotated_outline_gets_outline_kind_unknown_bucket() {
        // Outline annotation on a class whose LHS doesn't match any
        // known synthetic shape — the annotation IS authoritative
        // (the method IS outlined), but we can't fill the kind axis
        // from the class name alone. Joint-signal anchor upgrades
        // the bucket from Unknown to OutlineKindUnknown so analyst
        // output can distinguish "no signal at all" from "verified
        // outline, kind ambiguous due to R8 minimal naming".
        let body = "\
com.developer.Code -> dd:
    1:1:int doThing() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
";
        let s = OutlineSet::parse(body);
        assert_eq!(s.outlined_count(), 1);
        assert_eq!(
            s.outlined_kind("dd", "a"),
            Some(SyntheticKind::OutlineKindUnknown),
        );
    }

    #[test]
    fn mapping_minimal_named_synthetic_with_outline_annotation_attributed() {
        // The production-default R8 emit shape: `<context>$<id>`.
        // No verbose ExternalSynthetic infix; the kind discriminator
        // is gone. Joint-signal: the outline annotation is present,
        // so attribute as OutlineKindUnknown (verified outline,
        // kind unrecoverable from this mapping alone).
        let body = "\
fox.droidsaw.r8fixture.FixtureCallers$0 -> a.a:
    1:5:java.lang.String m(java.lang.String,int,java.lang.String):0:4 -> a
      # {\"id\":\"com.android.tools.r8.synthesized\"}
      # {\"id\":\"com.android.tools.r8.outline\"}
";
        let s = OutlineSet::parse(body);
        assert_eq!(s.outlined_count(), 1);
        assert_eq!(
            s.outlined_kind("a.a", "a"),
            Some(SyntheticKind::OutlineKindUnknown),
        );
    }

    #[test]
    fn mapping_verbose_name_still_pins_specific_kind() {
        // Joint-signal must NOT override the verbose path. Classes
        // with `$$ExternalSynthetic<Kind>` infix get attributed to
        // the specific kind, not collapsed into OutlineKindUnknown.
        let body = "\
a.A$$ExternalSyntheticOutline$0 -> a0:
    1:1:int f() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
b.B$$ExternalSyntheticBUOutline$0 -> b0:
    1:1:int f() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
";
        let s = OutlineSet::parse(body);
        assert_eq!(s.outlined_kind("a0", "a"), Some(SyntheticKind::Outline));
        assert_eq!(
            s.outlined_kind("b0", "a"),
            Some(SyntheticKind::BottomUpOutline),
        );
    }

    #[test]
    fn mapping_legacy_named_still_pins_legacy_kind() {
        // Joint-signal must NOT override the legacy path either.
        let body = "\
com.example.GeneratedOutlineSupport -> g:
    1:1:int outline() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
";
        let s = OutlineSet::parse(body);
        assert_eq!(
            s.outlined_kind("g", "a"),
            Some(SyntheticKind::LegacyGeneratedOutlineSupport),
        );
    }

    #[test]
    fn outlined_kind_counts_sum_to_outlined_count() {
        // Invariant: every outlined-method tuple gets exactly one
        // kind attribution; the counts map sums to outlined_count.
        let body = "\
a.A$$ExternalSyntheticOutline$0 -> a0:
    1:1:int f() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
b.B$$ExternalSyntheticOutline$0 -> b0:
    1:1:int f() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
c.C$$ExternalSyntheticApiModelOutline$0 -> c0:
    1:1:int f() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
d.D -> d0:
    1:1:int f() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
";
        let s = OutlineSet::parse(body);
        let counts = s.outlined_kind_counts();
        assert_eq!(counts.get(&SyntheticKind::Outline).copied(), Some(2));
        assert_eq!(counts.get(&SyntheticKind::ApiModelOutline).copied(), Some(1));
        // `d.D` has the outline annotation but no recognisable
        // synthetic infix on its name — joint-signal upgrades from
        // Unknown to OutlineKindUnknown.
        assert_eq!(
            counts.get(&SyntheticKind::OutlineKindUnknown).copied(),
            Some(1),
        );
        let sum: usize = counts.values().sum();
        assert_eq!(sum, s.outlined_count());
    }

    #[test]
    fn classifier_does_not_match_id_substring_in_value_field() {
        // Adversarial: a developer class name containing
        // "$$ExternalSyntheticOutline" as a verbatim PARAM type or
        // generic argument (improbable but possible in unminified
        // input). The class record's LHS classification kicks in
        // only on the OUTER class name, so the embedded substring
        // in a method param doesn't classify the parent class as
        // a specific kind.
        //
        // With the joint-signal anchor: the outline annotation IS
        // present + the LHS class didn't match any kind discriminator
        // → bucket is OutlineKindUnknown (verified outline, kind
        // ambiguous), not Unknown. The parent class being developer
        // code is implausible per R8's emit contract — but if the
        // annotation lands, the joint signal is sound regardless.
        let body = "\
com.developer.MyClass -> mm:
    1:1:int foo($$ExternalSyntheticOutline$0 helper) -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
";
        let s = OutlineSet::parse(body);
        assert_eq!(
            s.outlined_kind("mm", "a"),
            Some(SyntheticKind::OutlineKindUnknown),
        );
    }

    #[test]
    fn per_kind_match_report_attributes_matches() {
        let body = "\
a.A$$ExternalSyntheticOutline$0 -> a0:
    1:1:int f() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
a.A$$ExternalSyntheticOutline$1 -> a1:
    1:1:int f() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
b.B$$ExternalSyntheticBUOutline$0 -> b0:
    1:1:int f() -> a
    # {\"id\":\"com.android.tools.r8.outline\"}
";
        let s = OutlineSet::parse(body);
        // Recogniser fires for a0.a and b0.a, misses a1.a.
        let mut fired: BTreeSet<(String, String)> = BTreeSet::new();
        fired.insert(("a0".to_string(), "a".to_string()));
        fired.insert(("b0".to_string(), "a".to_string()));

        let report = s.per_kind_match_report(&fired);
        // Stable report order; every kind row present.
        assert_eq!(report.len(), SyntheticKind::report_order().len());
        // Find Outline row: 2 annotated, 1 matched.
        let outline_row = report
            .iter()
            .find(|(k, _, _)| *k == SyntheticKind::Outline)
            .expect("Outline row present");
        assert_eq!(outline_row.1, 2);
        assert_eq!(outline_row.2, 1);
        // BottomUpOutline row: 1 annotated, 1 matched.
        let bu_row = report
            .iter()
            .find(|(k, _, _)| *k == SyntheticKind::BottomUpOutline)
            .expect("BottomUpOutline row present");
        assert_eq!(bu_row.1, 1);
        assert_eq!(bu_row.2, 1);
        // Untouched kinds zero-zero.
        let cov_row = report
            .iter()
            .find(|(k, _, _)| *k == SyntheticKind::CovariantOutline)
            .expect("CovariantOutline row present");
        assert_eq!(cov_row.1, 0);
        assert_eq!(cov_row.2, 0);
    }

    #[test]
    fn report_order_covers_all_variants() {
        // Sentinel: catch missed enum variants if a new kind is
        // added to SyntheticKind without updating report_order.
        let order = SyntheticKind::report_order();
        assert_eq!(order.len(), 10);
        // Each variant appears exactly once.
        let mut seen: BTreeSet<SyntheticKind> = BTreeSet::new();
        for &k in order {
            assert!(seen.insert(k), "duplicate variant in report_order: {k:?}");
        }
    }

    #[test]
    fn synthetic_kind_label_stable_for_all_variants() {
        // Stability: harness output and analyst greps key off
        // these labels; they must remain identical to the R8 source
        // SyntheticNaming.java enum spellings (with the local
        // OUTLINE_KIND_UNKNOWN + UNKNOWN labels naming the
        // joint-signal / no-signal cases respectively).
        assert_eq!(SyntheticKind::Outline.label(), "OUTLINE");
        assert_eq!(SyntheticKind::CovariantOutline.label(), "COVARIANT_OUTLINE");
        assert_eq!(SyntheticKind::ApiModelOutline.label(), "API_MODEL_OUTLINE");
        assert_eq!(
            SyntheticKind::NonStartupInStartupOutline.label(),
            "NON_STARTUP_IN_STARTUP_OUTLINE",
        );
        assert_eq!(SyntheticKind::BottomUpOutline.label(), "BOTTOM_UP_OUTLINE");
        assert_eq!(SyntheticKind::ObjectCloneOutline.label(), "OBJECT_CLONE_OUTLINE");
        assert_eq!(
            SyntheticKind::LegacyGeneratedOutlineSupport.label(),
            "LEGACY_GENERATED_OUTLINE_SUPPORT",
        );
        assert_eq!(SyntheticKind::EnumUnboxing.label(), "ENUM_UNBOXING");
        assert_eq!(
            SyntheticKind::OutlineKindUnknown.label(),
            "OUTLINE_KIND_UNKNOWN",
        );
        assert_eq!(SyntheticKind::Unknown.label(), "UNKNOWN");
    }

    #[test]
    fn synthetic_kind_classifier_recognises_enum_unboxing() {
        // R8's ENUM_UNBOXING_HELPER emit shape: the original boxed
        // enum host class with the `$EnumUnboxingLocalUtility`
        // suffix. Both the AndroidX example surfaced by the F-Droid
        // sweep and a generic developer-package shape classify.
        assert_eq!(
            classify_synthetic_kind("androidx.work.NetworkType$EnumUnboxingLocalUtility"),
            SyntheticKind::EnumUnboxing,
        );
        assert_eq!(
            classify_synthetic_kind("com.foo.MyEnum$EnumUnboxingLocalUtility"),
            SyntheticKind::EnumUnboxing,
        );
    }

    #[test]
    fn synthetic_kind_classifier_enum_unboxing_anchored_on_trailing_segment() {
        // Developer class whose name embeds `EnumUnboxingLocalUtility`
        // as a substring (no `$` boundary) must not bucket as
        // EnumUnboxing — the suffix anchor requires the `$` inner-
        // class separator + a trailing-position match.
        assert_eq!(
            classify_synthetic_kind("com.foo.MyEnumUnboxingLocalUtilityWrapper"),
            SyntheticKind::Unknown,
        );
        // `$`-boundary present but the segment extends past the
        // suffix — `ends_with` correctly rejects this shape.
        assert_eq!(
            classify_synthetic_kind("com.foo.X$EnumUnboxingLocalUtilityHelper"),
            SyntheticKind::Unknown,
        );
        // Package-segment embed only — the last dot-segment doesn't
        // carry the suffix, so it doesn't classify.
        assert_eq!(
            classify_synthetic_kind("com.EnumUnboxingLocalUtility.RealClass"),
            SyntheticKind::Unknown,
        );
    }
}
