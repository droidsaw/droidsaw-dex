//! Canonical-shape parser for R8Origin markers in droidsaw
//! decompile output.
//!
//! Two recogniser-emit shapes are accepted:
//!
//!   bytecode-only (the production path; no oracle):
//!   `/* @droidsaw R8Origin(StructurallyOutlineLike, helper=<L...;>-><name>, callers=<digits>, confidence=<digits>) */`
//!
//!   mapping-confirmed (test harness wired in an OutlineOracle):
//!   `/* @droidsaw R8Origin(BlockOutlinedHelper, mapping_confirmed=<true|false>, helper=<L...;>-><name>, callers=<digits>, confidence=<digits>) */`
//!
//! The legacy `BlockOutlinedTrampoline` variant is also accepted as
//! a forward-compat hook (the variant is reserved but not produced
//! by the current recogniser).
//!
//! A decompile output that contains marker-shaped text inside a
//! string constant or method name could inflate marker counts in
//! consumers that scan with naive substring matching. This module
//! provides a strict line parser that mirrors the production
//! emit shape exactly.
//!
//! Anchored as the line, trimmed, must START with the frame
//! prefix and END with the frame suffix, with the body matching
//! the 4- or 5-field shape (variant + optional mapping_confirmed
//!     + helper-class + caller count + confidence). All deviations
//!     return `None`.

const FRAME_PREFIX: &str = "/* @droidsaw R8Origin(";
const FRAME_SUFFIX: &str = ") */";

/// Parsed contents of one canonical marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockOutlinedMarker<'a> {
    /// Variant identifier — `BlockOutlinedHelper`,
    /// `BlockOutlinedTrampoline`, `StructurallyOutlineLike`, or a
    /// future suffix.
    pub variant: &'a str,
    /// Mapping-confirmed flag, present only on `BlockOutlinedHelper`
    /// (test harness wired in an OutlineOracle). `None` for the
    /// bytecode-only `StructurallyOutlineLike` variant emitted by
    /// the production path.
    pub mapping_confirmed: Option<bool>,
    /// Helper class descriptor (`Lh1;`-style DEX descriptor).
    pub helper_class: &'a str,
    /// Helper method name (e.g. `a`, `outline0`, `<clinit>`).
    pub helper_method: &'a str,
    /// Caller count reported by the recogniser.
    pub callers: u64,
    /// Confidence percentage reported by the recogniser.
    pub confidence: u64,
}

/// Parse one line, returning a `BlockOutlinedMarker` if the line
/// matches the canonical shape exactly. Returns `None` for any
/// deviation. Accepts both the 4-field bytecode-only shape and the
/// 5-field mapping-confirmed shape.
pub fn parse_block_outlined_marker(line: &str) -> Option<BlockOutlinedMarker<'_>> {
    let trimmed = line.trim();
    let after_prefix = trimmed.strip_prefix(FRAME_PREFIX)?;
    let body = after_prefix.strip_suffix(FRAME_SUFFIX)?;

    let parts: Vec<&str> = body.split(", ").collect();
    if parts.len() != 4 && parts.len() != 5 {
        return None;
    }

    // Literal allowlist of variant names. A prefix-match
    // (`starts_with("BlockOutlined")`) plus alphanum+`_` would admit
    // attacker-crafted names like `BlockOutlined_Helper` or
    // `BlockOutlinedXYZ`; downstream consumers that prefix-match on
    // `m.variant` would treat them as real markers. Pin every
    // accepted variant name explicitly — a future variant addition
    // requires updating this list, which is the right cost.
    let variant = parts[0];
    if !matches!(
        variant,
        "BlockOutlinedHelper" | "BlockOutlinedTrampoline" | "StructurallyOutlineLike"
    ) {
        return None;
    }

    // Optional `mapping_confirmed=` slot. Only valid on the
    // BlockOutlinedHelper variant — StructurallyOutlineLike is the
    // bytecode-only path and cannot carry a confirmation field.
    let (mapping_confirmed, helper_field_idx) = if parts.len() == 5 {
        if variant != "BlockOutlinedHelper" {
            return None;
        }
        let mc_str = parts[1].strip_prefix("mapping_confirmed=")?;
        let mc_val = match mc_str {
            "true" => true,
            "false" => false,
            _ => return None,
        };
        (Some(mc_val), 2)
    } else {
        if variant == "BlockOutlinedHelper" {
            // BlockOutlinedHelper without the mapping_confirmed slot
            // is a shape error — the emit always pairs the variant
            // with the field.
            return None;
        }
        (None, 1)
    };

    let helper_field = parts[helper_field_idx].strip_prefix("helper=")?;
    let (helper_class, helper_method) = helper_field.split_once("->")?;
    if !is_valid_class_descriptor(helper_class) {
        return None;
    }
    if helper_method.is_empty()
        || !helper_method.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '<' || c == '>' || c == '$'
        })
    {
        return None;
    }

    let callers_str = parts[helper_field_idx + 1].strip_prefix("callers=")?;
    let callers = parse_u64(callers_str)?;

    let confidence_str = parts[helper_field_idx + 2].strip_prefix("confidence=")?;
    let confidence = parse_u64(confidence_str)?;

    Some(BlockOutlinedMarker {
        variant,
        mapping_confirmed,
        helper_class,
        helper_method,
        callers,
        confidence,
    })
}

/// Count canonical `BlockOutlined` markers across all lines of `s`.
/// A line that fails the strict shape is silently dropped (which
/// is the point — string constants, method names, and unrelated
/// comment text cannot inflate the count).
pub fn count_block_outlined_markers(s: &str) -> usize {
    let mut count = 0usize;
    for line in s.lines() {
        if parse_block_outlined_marker(line).is_some() {
            count = count.saturating_add(1);
        }
    }
    count
}

/// Validate a DEX class descriptor of the shape `L<chars>;`. Inner
/// chars are ASCII alphanumeric or `/`, `$`, `_`. Rejects empty
/// inner, missing terminator, internal whitespace, non-ASCII.
pub fn is_valid_class_descriptor(desc: &str) -> bool {
    let Some(inner) = desc.strip_prefix('L').and_then(|s| s.strip_suffix(';')) else {
        return false;
    };
    !inner.is_empty()
        && inner.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '/' || c == '$' || c == '_'
        })
}

/// Convert an obfuscated helper class descriptor (`La;` or `Lk0/b;`)
/// to the source-form key R8's mapping records (`a` or `k0.b`).
///
/// Returns empty string for invalid descriptors. Callers should treat
/// empty as a lookup-miss sentinel — the empty key will never match
/// any real mapping-record key, so the collision-shadow attack where
/// `La.b;` after `/`->`.` would shadow the key for `La/b;` is
/// neutralised by lookup miss rather than producing a false positive.
pub fn descriptor_to_mapping_key(descriptor: &str) -> String {
    if !is_valid_class_descriptor(descriptor) {
        return String::new();
    }
    let trimmed = descriptor
        .strip_prefix('L')
        .and_then(|s| s.strip_suffix(';'))
        .unwrap_or(descriptor);
    trimmed.replace('/', ".")
}

fn parse_u64(s: &str) -> Option<u64> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structurally_outline_like_marker_parses() {
        // Production-emit shape: 4 fields, no mapping_confirmed slot.
        let line = "/* @droidsaw R8Origin(StructurallyOutlineLike, helper=La;->a, callers=3, confidence=100) */";
        let m = parse_block_outlined_marker(line).expect("canonical");
        assert_eq!(m.variant, "StructurallyOutlineLike");
        assert_eq!(m.mapping_confirmed, None);
        assert_eq!(m.helper_class, "La;");
        assert_eq!(m.helper_method, "a");
        assert_eq!(m.callers, 3);
        assert_eq!(m.confidence, 100);
    }

    #[test]
    fn block_outlined_helper_mapping_confirmed_true_parses() {
        // Mapping-confirmed shape (test harness with OutlineOracle):
        // 5 fields, mapping_confirmed=true.
        let line = "/* @droidsaw R8Origin(BlockOutlinedHelper, mapping_confirmed=true, helper=La;->a, callers=3, confidence=100) */";
        let m = parse_block_outlined_marker(line).expect("canonical mapping-confirmed");
        assert_eq!(m.variant, "BlockOutlinedHelper");
        assert_eq!(m.mapping_confirmed, Some(true));
        assert_eq!(m.helper_class, "La;");
    }

    #[test]
    fn block_outlined_helper_mapping_confirmed_false_parses() {
        // Allowlisted-disagreement shape: oracle says NOT outlined,
        // harness elected to tolerate via allowlist.
        let line = "/* @droidsaw R8Origin(BlockOutlinedHelper, mapping_confirmed=false, helper=La;->a, callers=27, confidence=100) */";
        let m = parse_block_outlined_marker(line).expect("canonical mapping-disagreed");
        assert_eq!(m.variant, "BlockOutlinedHelper");
        assert_eq!(m.mapping_confirmed, Some(false));
    }

    #[test]
    fn block_outlined_helper_without_mapping_confirmed_rejected() {
        // The 4-field BlockOutlinedHelper shape is no longer emitted —
        // the production path emits StructurallyOutlineLike; the
        // mapping-paired path emits BlockOutlinedHelper WITH the
        // mapping_confirmed slot. Reject the old 4-field form.
        let line = "/* @droidsaw R8Origin(BlockOutlinedHelper, helper=La;->a, callers=3, confidence=100) */";
        assert!(parse_block_outlined_marker(line).is_none());
    }

    #[test]
    fn structurally_outline_like_with_mapping_confirmed_rejected() {
        // StructurallyOutlineLike is the bytecode-only path — it
        // cannot carry a mapping_confirmed= field.
        let line = "/* @droidsaw R8Origin(StructurallyOutlineLike, mapping_confirmed=true, helper=La;->a, callers=3, confidence=100) */";
        assert!(parse_block_outlined_marker(line).is_none());
    }

    #[test]
    fn mapping_confirmed_non_bool_rejected() {
        let line = "/* @droidsaw R8Origin(BlockOutlinedHelper, mapping_confirmed=maybe, helper=La;->a, callers=3, confidence=100) */";
        assert!(parse_block_outlined_marker(line).is_none());
    }

    #[test]
    fn trampoline_variant_parses() {
        // Forward-compat hook: BlockOutlinedTrampoline is reserved
        // but not currently emitted; the parser accepts the 4-field
        // shape for it (no mapping_confirmed slot).
        let line = "/* @droidsaw R8Origin(BlockOutlinedTrampoline, helper=Lhf1;->s, callers=24, confidence=100) */";
        let m = parse_block_outlined_marker(line).expect("trampoline");
        assert_eq!(m.variant, "BlockOutlinedTrampoline");
        assert_eq!(m.mapping_confirmed, None);
        assert_eq!(m.callers, 24);
    }

    #[test]
    fn marker_inside_string_literal_rejected() {
        let line = r#"    String s = "/* @droidsaw R8Origin(StructurallyOutlineLike, helper=Levil;->a, callers=99, confidence=100) */";"#;
        assert!(parse_block_outlined_marker(line).is_none());
    }

    #[test]
    fn extra_field_rejected() {
        let line = "/* @droidsaw R8Origin(StructurallyOutlineLike, helper=La;->a, callers=3, confidence=100, extra=x) */";
        assert!(parse_block_outlined_marker(line).is_none());
    }

    #[test]
    fn missing_field_rejected() {
        let line = "/* @droidsaw R8Origin(StructurallyOutlineLike, helper=La;->a, callers=3) */";
        assert!(parse_block_outlined_marker(line).is_none());
    }

    #[test]
    fn invalid_descriptor_rejected() {
        let line = "/* @droidsaw R8Origin(StructurallyOutlineLike, helper=Lab cd;->a, callers=2, confidence=100) */";
        assert!(parse_block_outlined_marker(line).is_none());
    }

    #[test]
    fn non_block_outlined_variant_rejected() {
        let line = "/* @droidsaw R8Origin(MethodInlined, helper=La;->a, callers=0, confidence=40) */";
        assert!(parse_block_outlined_marker(line).is_none());
    }

    #[test]
    fn underscore_variant_name_attack_rejected() {
        // Attacker-crafted variant names like `BlockOutlined_Helper`
        // or `BlockOutlinedXYZ` would slip past a `starts_with` +
        // alphanum-or-underscore gate. The literal allowlist rejects
        // them — downstream consumers that prefix-match on
        // `m.variant` can rely on it being exactly one of the
        // recognised names.
        let underscore = "/* @droidsaw R8Origin(BlockOutlined_Helper, mapping_confirmed=true, helper=La;->a, callers=3, confidence=100) */";
        assert!(parse_block_outlined_marker(underscore).is_none());
        let bogus = "/* @droidsaw R8Origin(BlockOutlinedXYZ, helper=La;->a, callers=3, confidence=100) */";
        assert!(parse_block_outlined_marker(bogus).is_none());
        let typo = "/* @droidsaw R8Origin(Structurally_OutlineLike, helper=La;->a, callers=3, confidence=100) */";
        assert!(parse_block_outlined_marker(typo).is_none());
    }

    #[test]
    fn non_numeric_counter_rejected() {
        let line = "/* @droidsaw R8Origin(StructurallyOutlineLike, helper=La;->a, callers=abc, confidence=100) */";
        assert!(parse_block_outlined_marker(line).is_none());
    }

    #[test]
    fn count_works() {
        let body = "
/* @droidsaw R8Origin(StructurallyOutlineLike, helper=La;->a, callers=3, confidence=100) */
String s = \"@droidsaw R8Origin(StructurallyOutlineLike,helper=Levil;\";
/* @droidsaw R8Origin(BlockOutlinedHelper, mapping_confirmed=true, helper=Lb;->m, callers=5, confidence=80) */
";
        assert_eq!(count_block_outlined_markers(body), 2);
    }

    #[test]
    fn descriptor_to_mapping_key_works() {
        assert_eq!(descriptor_to_mapping_key("La;"), "a");
        assert_eq!(descriptor_to_mapping_key("Lk0/b;"), "k0.b");
        assert_eq!(
            descriptor_to_mapping_key("Lcom/instagram/Foo;"),
            "com.instagram.Foo"
        );
    }

    #[test]
    fn descriptor_to_mapping_key_rejects_dot_in_inner() {
        // Attack B: `La.b;` after `/`->`.` would have shadowed
        // mapping key `a.b` produced by `La/b;`. The validating
        // gate now rejects `.` in the inner descriptor and returns
        // the empty-string sentinel.
        assert_eq!(descriptor_to_mapping_key("La.b;"), "");
        assert_ne!(descriptor_to_mapping_key("La.b;"), "a.b");
    }

    #[test]
    fn descriptor_to_mapping_key_rejects_missing_l_prefix() {
        assert_eq!(descriptor_to_mapping_key("a/b;"), "");
        assert_eq!(descriptor_to_mapping_key(""), "");
    }

    #[test]
    fn descriptor_to_mapping_key_rejects_missing_terminator() {
        assert_eq!(descriptor_to_mapping_key("La/b"), "");
        assert_eq!(descriptor_to_mapping_key("L"), "");
    }

    #[test]
    fn collision_attack_neutralised() {
        // The Attack B collision: `La.b;` (malformed, dot-bearing)
        // must not produce the same mapping key as the legitimate
        // `La/b;`. With the validating gate, the former returns the
        // empty sentinel while the latter returns `a.b` — distinct,
        // so a lookup keyed on the malformed descriptor misses
        // rather than shadowing the legitimate entry.
        let attacker = descriptor_to_mapping_key("La.b;");
        let victim = descriptor_to_mapping_key("La/b;");
        assert_eq!(attacker, "");
        assert_eq!(victim, "a.b");
        assert_ne!(attacker, victim);
    }
}
