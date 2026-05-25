//! Adversarial PoC ratchet — smali-forged masquerade (positive control).
//!
//! Companion to `r8_family_prefix_masquerade_fixture_ratchet.rs`. That
//! fixture is the **negative control**: kotlinc-emitted classes lack
//! `ACC_SYNTHETIC`, so the recogniser's class-flag gate (`r8_inversion.rs`
//! around line 846) rejects the naive name-only masquerade. This fixture
//! is the **positive control**: an attacker who controls bytecode emit
//! can forge `ACC_SYNTHETIC` AND craft a `$$ExternalSyntheticOutline0`
//! descriptor suffix, defeating both the family-prefix filter AND the
//! recogniser's structural gates. The recogniser still fires — but at
//! the low-trust `StructurallyOutlineLike` variant, because there is no
//! paired mapping.txt to elevate the marker to `BlockOutlinedHelper`
//! via `elevate_with_oracle`. The marker is honest about its uncertainty.
//!
//! The "smali" name is historical — the attack is direct bytecode
//! authoring; the assembler is incidental. This fixture authors bytes
//! by mutating an in-tree minimal DEX in-process and re-emitting through
//! `droidsaw_dex::emit_dex::emit_dex`. No external assembler required;
//! the test runs everywhere with zero extra toolchain.
//!
//! # What the forgery does (sequence)
//!
//! Input: `tests/fixtures/classes_named.dex` (a 1236-byte D8 output
//! containing a single `LMinimalNamed;` class with a static `accumulate`
//! method). The mutations:
//!
//! 1. **Descriptor surgery.** Replace the `LMinimalNamed;` entry in the
//!    strings pool **in-place** with `Landroidx/internal/Forged$$Ex\
//!    ternalSyntheticOutline0;`. In-place mutation preserves the
//!    `type_idxs` monotonicity emit_dex demands — same numeric string
//!    index, new content. The descriptor (a) lives in the `androidx`
//!    namespace, so it lexically matches the `KNOWN_FP_FAMILY` allow
//!    list, (b) carries the `$$ExternalSyntheticOutline` infix the
//!    recogniser uses as a synthetic-source signal.
//!
//! 2. **`ACC_SYNTHETIC` forgery.** OR `0x1000` into the helper's
//!    `class_def.access_flags`. R8's `SyntheticItems` sets this bit on
//!    every class it synthesises; bytecode-controlling attackers can
//!    set it too. No runtime verifier check distinguishes the two
//!    sources (ART simply trusts the flag).
//!
//! 3. **Helper body rewrite.** Replace `accumulate`'s original 11-insn
//!    body (which contains `IfGe` and `Goto` — both rejected by
//!    `body_is_outline_eligible`) with a 2-insn body
//!    `const-wide/16 v0, 0; return-wide v0`. Both opcodes are
//!    outline-eligible. This shape mirrors what R8's basic Outliner
//!    actually extracts (straight-line constant + arithmetic + return).
//!
//! 4. **Mint two caller methods.** Add `call_a` and `call_b` static
//!    direct methods to the helper class_data, each containing
//!    `const-4 v0, 0; invoke-static accumulate(v0); return-void`. R8's
//!    `OutlineOptions.threshold` is 20; the recogniser's distinct-caller
//!    ladder admits at ≥ 2 with confidence 40. Two callers is the
//!    minimum that fires.
//!
//! 5. **PWNED witness string.** Add `"PWNED:smali-masquerade"` to the
//!    strings pool. The string survives in the emitted DEX's strings
//!    section even though the recogniser fires on the class — proving
//!    the family-prefix filter is **lens-blinding**, not **code-hiding**
//!    (an analyst running `strings` on the DEX still sees the witness).
//!
//! Re-emit via `emit_dex`. Re-parse the result. Run the production
//! recogniser pipeline (`decompile_class_with_census`) and scan the
//! output for `/* @droidsaw R8Origin(...) */` markers.
//!
//! # Assertions
//!
//! 1. **Re-parse succeeds.** Confirms the mutated IR + emit produces a
//!    spec-compliant DEX. emit_dex would have rejected internal
//!    inconsistencies (string-pool / type_idxs monotonicity, etc.)
//!    before bytes were produced; the re-parse confirms the bytes
//!    actually round-trip through `DexFile::parse`.
//!
//! 2. **Recogniser fires exactly once on the masquerade class.** The
//!    marker class is `Landroidx/internal/Forged$$ExternalSyntheticOu\
//!    tline0;`, method `accumulate`. Caller-methods `call_a` and
//!    `call_b` do NOT fire: each has 0 distinct callers itself, below
//!    the ≥ 2 admission floor.
//!
//! 3. **Marker variant is `StructurallyOutlineLike`** — the low-trust
//!    variant emitted when there is no `OutlineOracle` to elevate the
//!    marker via paired mapping. This is the load-bearing assertion of
//!    this fixture: an attacker who defeats every bytecode-level gate
//!    still cannot promote the marker to `BlockOutlinedHelper`, because
//!    that promotion requires a paired mapping.txt declaring the
//!    method outlined — and an attacker without control of the R8
//!    toolchain cannot forge the mapping.
//!
//! 4. **Confidence is 40.** Two distinct callers maps to the floor tier
//!    of `block_outlined_ladder_confidence`. If the recogniser starts
//!    emitting a higher tier on this shape, the ladder semantics
//!    drifted and the fixture catches it.
//!
//! 5. **`"PWNED:smali-masquerade"` survives.** Lens-blinding-not-
//!    code-hiding witness. `strings` on the emitted DEX still surfaces
//!    the literal.
//!
//! # What this fixture is NOT
//!
//! - **Not a security boundary regression test.** The recogniser's
//!   gates are FP-reduction heuristics, not access control. This
//!   fixture documents the limits of those heuristics on adversarial
//!   input; the security boundary is the paired-mapping oracle
//!   (`elevate_with_oracle`), and bypassing it requires forging an R8
//!   mapping.txt — out of scope for the bytecode-only attacker model.
//!
//! - **Not a fuzz harness.** One fixture, one attack pattern. Don't
//!   expand into a suite of adversarial bytecode without an explicit
//!   scope.

use droidsaw_dex::decode::{CodeItem, EncodedMethod, Instruction, PoolIndex, RegList};
use droidsaw_dex::emit_dex::emit_dex;
use droidsaw_dex::ids::{MethodIdItem, MethodIdx, ProtoIdx, StringIdx};
use droidsaw_dex::opcodes::Opcode;
use droidsaw_dex::parser::DexFile;
use droidsaw_dex::r8_inversion::build_trampoline_census;
use droidsaw_dex::DexString;

mod common;
use common::r8_canonical_marker::parse_block_outlined_marker;

const MASQUERADE_DESCRIPTOR: &str = "Landroidx/internal/Forged$$ExternalSyntheticOutline0;";
const PWNED_WITNESS: &str = "PWNED:smali-masquerade";

const ACC_PUBLIC: u32 = 0x0001;
const ACC_STATIC: u32 = 0x0008;
const ACC_SYNTHETIC: u32 = 0x1000;

#[test]
fn r8_smali_masquerade_forgery_fires_low_trust_marker() {
    let forged_bytes = build_masquerade_dex();
    let dex = DexFile::parse(&forged_bytes, None)
        .unwrap_or_else(|e| panic!("re-parse forged DEX: {e:?}"));
    assert_eq!(dex.class_defs.len(), 1, "expected 1 masquerade class");
    let cd = &dex.class_defs[0];
    let desc = &dex.type_descriptors[cd.class_idx.0 as usize];
    assert_eq!(
        desc, MASQUERADE_DESCRIPTOR,
        "masquerade descriptor did not survive emit→parse",
    );
    assert_eq!(
        cd.access_flags & ACC_SYNTHETIC,
        ACC_SYNTHETIC,
        "ACC_SYNTHETIC bit not set on masquerade class (got 0x{:04x})",
        cd.access_flags,
    );

    let census = build_trampoline_census(&dex);
    let mut markers = Vec::new();
    for cd in &dex.class_defs {
        if cd.class_data_off == 0 {
            continue;
        }
        let out = droidsaw_dex::classes::decompile_class_with_census(
            &dex,
            &forged_bytes,
            cd,
            &census,
        );
        for line in out.lines() {
            if let Some(marker) = parse_block_outlined_marker(line) {
                markers.push((
                    marker.variant.to_string(),
                    marker.helper_class.to_string(),
                    marker.helper_method.to_string(),
                    marker.callers,
                    marker.confidence,
                ));
            }
        }
    }

    assert_eq!(
        markers.len(),
        1,
        "expected exactly 1 recogniser marker on the masquerade class, got {}:\n  {}",
        markers.len(),
        markers
            .iter()
            .map(|(v, c, m, ca, co)| format!("{v} {c}->{m} callers={ca} confidence={co}"))
            .collect::<Vec<_>>()
            .join("\n  "),
    );

    let (variant, helper_class, helper_method, callers, confidence) = &markers[0];
    assert_eq!(
        variant, "StructurallyOutlineLike",
        "marker variant must be the low-trust StructurallyOutlineLike on bytecode-only forgery (no mapping oracle); \
         got {variant}. If the recogniser is now emitting BlockOutlinedHelper on bytecode-only input, \
         the oracle-required elevation contract has regressed.",
    );
    assert_eq!(helper_class, MASQUERADE_DESCRIPTOR);
    assert_eq!(helper_method, "accumulate");
    assert_eq!(
        *callers, 2,
        "expected 2 distinct callers (call_a, call_b); got {callers}",
    );
    assert_eq!(
        *confidence, 40,
        "expected confidence 40 (floor tier of block_outlined_ladder_confidence at ≥2 callers); got {confidence}",
    );

    let pwned_survived = dex
        .strings
        .iter()
        .any(|s| s.as_str_lossy().contains(PWNED_WITNESS));
    assert!(
        pwned_survived,
        "PWNED witness '{PWNED_WITNESS}' did not survive emit→parse — \
         lens-blinding-not-code-hiding invariant lost",
    );
}

fn build_masquerade_dex() -> Vec<u8> {
    let bytes = include_bytes!("fixtures/classes_named.dex");
    let mut dex = DexFile::parse(bytes, None).expect("parse base fixture");

    let helper_class_idx = dex
        .type_descriptors
        .iter()
        .position(|d| d == "LMinimalNamed;")
        .expect("LMinimalNamed; in base fixture");
    let helper_method_idx = dex
        .methods
        .iter()
        .position(|m| {
            dex.type_descriptors[m.class_idx.0 as usize] == "LMinimalNamed;"
                && dex.get_string(m.name_idx).ok() == Some("accumulate")
        })
        .expect("LMinimalNamed;->accumulate in base fixture");

    let lm_string_idx = dex
        .strings
        .iter()
        .position(|s| s.as_str_lossy() == "LMinimalNamed;")
        .expect("LMinimalNamed; in strings pool");
    dex.strings[lm_string_idx] = DexString::from_decoded_str(MASQUERADE_DESCRIPTOR);
    dex.type_descriptors[helper_class_idx] = MASQUERADE_DESCRIPTOR.to_string();
    for cd in &mut dex.class_defs {
        if cd.class_idx.0 as usize == helper_class_idx {
            cd.access_flags |= ACC_SYNTHETIC;
        }
    }

    let accumulate_code_off = {
        let class_def = dex
            .class_defs
            .iter()
            .find(|cd| cd.class_idx.0 as usize == helper_class_idx)
            .expect("class_def for masquerade class");
        let class_data = dex
            .class_datas
            .get(&class_def.class_data_off)
            .expect("class_data for masquerade class");
        class_data
            .direct_methods
            .iter()
            .find(|em| em.method_idx.0 as usize == helper_method_idx)
            .expect("EncodedMethod for accumulate")
            .code_off
    };
    let outline_eligible_body = CodeItem {
        registers_size: 2,
        ins_size: 1,
        outs_size: 0,
        debug_info_off: 0,
        instructions: vec![
            Instruction {
                addr: 0,
                op: Opcode::ConstWide16,
                size: 2,
                dst: Some(0),
                src: RegList::empty(),
                literal: 0,
                target: None,
                pool_idx: None,
            },
            Instruction {
                addr: 2,
                op: Opcode::ReturnWide,
                size: 1,
                dst: Some(0),
                src: RegList::empty(),
                literal: 0,
                target: None,
                pool_idx: None,
            },
        ],
        tries: vec![],
        catch_handlers: vec![],
        payloads: Default::default(),
        invariant_violations: vec![],
    };
    dex.code_items.insert(accumulate_code_off, outline_eligible_body);

    dex.strings.push(DexString::from_decoded_str(PWNED_WITNESS));
    dex.string_data_offs.push(0);

    let void_proto_idx = dex
        .protos
        .iter()
        .position(|p| dex.get_string(p.shorty_idx).ok() == Some("V"))
        .expect("()V proto in base fixture") as u32;
    let helper_type_idx = dex
        .methods
        .iter()
        .find(|m| dex.type_descriptors[m.class_idx.0 as usize] == MASQUERADE_DESCRIPTOR)
        .map(|m| m.class_idx)
        .expect("masquerade class_idx after rewrite");

    let mut new_methods: Vec<MethodIdx> = Vec::new();
    let mut new_code_offs: Vec<u32> = Vec::new();
    let mut next_synth_code_off: u32 = 0xFFF0_0000;
    for name in ["call_a", "call_b"] {
        let name_idx = StringIdx(dex.strings.len() as u32);
        dex.strings.push(DexString::from_decoded_str(name));
        dex.string_data_offs.push(0);
        let method_id = MethodIdItem {
            class_idx: helper_type_idx,
            proto_idx: ProtoIdx(void_proto_idx),
            name_idx,
        };
        let method_idx = MethodIdx(dex.methods.len() as u32);
        dex.methods.push(method_id);
        let caller_body = CodeItem {
            registers_size: 1,
            ins_size: 0,
            outs_size: 1,
            debug_info_off: 0,
            instructions: vec![
                Instruction {
                    addr: 0,
                    op: Opcode::Const4,
                    size: 1,
                    dst: Some(0),
                    src: RegList::empty(),
                    literal: 0,
                    target: None,
                    pool_idx: None,
                },
                Instruction {
                    addr: 1,
                    op: Opcode::InvokeStatic,
                    size: 3,
                    dst: None,
                    src: RegList::one(0),
                    literal: 0,
                    target: None,
                    pool_idx: Some(PoolIndex::Method(MethodIdx(helper_method_idx as u32))),
                },
                Instruction {
                    addr: 4,
                    op: Opcode::ReturnVoid,
                    size: 1,
                    dst: None,
                    src: RegList::empty(),
                    literal: 0,
                    target: None,
                    pool_idx: None,
                },
            ],
            tries: vec![],
            catch_handlers: vec![],
            payloads: Default::default(),
            invariant_violations: vec![],
        };
        dex.code_items.insert(next_synth_code_off, caller_body);
        new_methods.push(method_idx);
        new_code_offs.push(next_synth_code_off);
        next_synth_code_off = next_synth_code_off.saturating_add(0x100);
    }

    let helper_class_data_off = dex
        .class_defs
        .iter()
        .find(|cd| cd.class_idx.0 as usize == helper_class_idx)
        .map(|cd| cd.class_data_off)
        .expect("helper class_data_off");
    let class_data = dex
        .class_datas
        .get_mut(&helper_class_data_off)
        .expect("helper class_data");
    for (i, midx) in new_methods.iter().enumerate() {
        class_data.direct_methods.push(EncodedMethod {
            method_idx: *midx,
            access_flags: ACC_PUBLIC | ACC_STATIC,
            code_off: new_code_offs[i],
        });
    }

    emit_dex(&dex).expect("emit_dex on forged DexFile")
}
