//! Structure-Preserving Representation (SPR) — obfuscation-invariant
//! Dalvik bytecode normalization.
//!
//! A method's instruction stream is tokenized through a ladder of
//! increasingly abstract representations so that *renaming* (R8 /
//! ProGuard minification) and encoding-form re-selection produce the
//! same token sequence. The ladder is BC → AR → NR → SPR → Fuzzy-SPR;
//! this module currently implements the bottom two rungs:
//!
//! - **BC** — concrete: full mnemonic (with width/encoding suffix),
//!   registers, immediate value, the *absolute* branch target, and the
//!   resolved pool reference. Name-dependent by design.
//! - **AR** — abstract: NOPs removed, the branch target reduced to a
//!   *direction* (forward / backward / zero), and the opcode's
//!   encoding-form suffix collapsed (so `invoke-virtual` and
//!   `invoke-virtual/range`, `const-string` and `const-string/jumbo`,
//!   the `const/{4,16,high16,…}` width family, etc. share one op-class).
//!   Registers and resolved names are preserved. AR is the rung at which
//!   R8's encoding-form choices (which it makes on register/index/value
//!   size, and which renaming/re-optimization perturbs) stop mattering.
//!
//! Name erasure (mapping a non-Android type to its Android-API
//! supertype, dropping method names) is the NR rung and is not yet
//! implemented; the structural skeleton produced here is what NR builds
//! on, and the test suite proves that skeleton is already independent of
//! type names.
//!
//! Operating contract: SPR consumes *canonical* application DEX. A
//! method whose decode desynced on an unmapped opcode byte
//! ([`CodeItemInvariantViolation::UnknownOpcodeByte`] — ODEX/quick or
//! unused bytes, which the tolerant decoder skips by one code unit while
//! the real instruction is wider) cannot be soundly tokenized: the
//! remainder of the stream is misaligned. Such a method is reported
//! [`Unencodable::DecodeDesync`] rather than emitting a fictional
//! partial sequence.

use core::fmt;

use crate::decode::{insn_format, CodeItem, CodeItemInvariantViolation, InsnFormat, Instruction, PoolIndex};
use crate::opcodes::Opcode;
use crate::parser::DexFile;

/// SPR ladder level. `Bc` and `Ar` are implemented; `Nr`/`Spr`/
/// `FuzzySpr` follow in later rungs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Concrete bytecode: full mnemonic, absolute branch targets,
    /// NOPs retained, resolved names. Name-dependent.
    Bc,
    /// Abstract representation: NOPs removed, branch targets reduced to
    /// a direction, encoding-form suffixes collapsed. Names preserved.
    Ar,
}

/// Why a method could not be encoded into a token sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unencodable {
    /// The method's instruction stream desynced on an unmapped opcode
    /// byte; tokenizing the misaligned remainder would be a fiction.
    DecodeDesync,
}

/// Branch operand at a given level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Branch {
    /// `target > addr` — a forward branch (AR spelling: `along`).
    Forward,
    /// `target < addr` — a backward branch (AR spelling: `back`).
    Backward,
    /// `target == addr` — a zero-offset self-branch.
    Zero,
    /// BC: the absolute resolved target code-unit address.
    Absolute(u32),
}

/// One normalized instruction token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Op-class spelling: at `Bc` the full mnemonic; at `Ar` the
    /// mnemonic truncated at its first `/` (encoding-form suffix
    /// collapsed).
    pub op: String,
    /// Register operands: the destination (if any) followed by the
    /// source registers, in order.
    pub regs: Vec<u16>,
    /// Immediate value, present only for formats that carry a real
    /// signed/zero-extended literal (not the range-invoke arg count,
    /// which shares the `literal` field in the decoded IR).
    pub lit: Option<i64>,
    /// Branch operand, present only for goto / if-* formats (switch and
    /// fill-array reference an out-of-band payload, not a branch).
    pub branch: Option<Branch>,
    /// Resolved pool reference (type descriptor, method/field
    /// signature, string value, or a kind tag for call sites /
    /// method-handle / method-type). `None` when the instruction
    /// carries no pool operand.
    pub pool: Option<String>,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.op)?;
        for r in &self.regs {
            write!(f, " v{r}")?;
        }
        if let Some(l) = self.lit {
            write!(f, " #{l}")?;
        }
        if let Some(b) = &self.branch {
            match b {
                Branch::Forward => write!(f, " along")?,
                Branch::Backward => write!(f, " back")?,
                Branch::Zero => write!(f, " zero")?,
                Branch::Absolute(a) => write!(f, " @{a}")?,
            }
        }
        if let Some(p) = &self.pool {
            write!(f, " {p}")?;
        }
        Ok(())
    }
}

/// Op-class spelling for `op` at `level`. At `Ar` the mnemonic's
/// encoding-form suffix (the part after the first `/`: `/from16`,
/// `/range`, `/jumbo`, `/high16`, `/2addr`, `/lit8`, …) is dropped, so
/// every encoding of the same semantic operation collapses to one
/// class. The Dalvik mnemonic grammar places the semantic op before the
/// `/` and the encoding modifier after it, so truncation is exact.
fn op_class(op: Opcode, level: Level) -> String {
    let m = op.mnemonic();
    match level {
        Level::Bc => m.to_string(),
        Level::Ar => m.split('/').next().unwrap_or(m).to_string(),
    }
}

/// True for formats whose `Instruction::literal` is a genuine immediate
/// value. `F3rc`/`F4rcc` also populate `literal` (with the arg count),
/// and `F31t` leaves it unused, so both are excluded.
fn format_has_immediate(fmt: InsnFormat) -> bool {
    matches!(
        fmt,
        InsnFormat::F11n
            | InsnFormat::F21s
            | InsnFormat::F21h
            | InsnFormat::F22s
            | InsnFormat::F22b
            | InsnFormat::F31i
            | InsnFormat::F51l
    )
}

/// True for goto / if-* formats — the ones whose `target` is a branch
/// destination. `F31t` (packed-switch / sparse-switch / fill-array)
/// stores a payload address in `target`, not a branch, so it is
/// excluded; its case targets live in `CodeItem.payloads`.
fn is_branch_format(fmt: InsnFormat) -> bool {
    matches!(
        fmt,
        InsnFormat::F10t
            | InsnFormat::F20t
            | InsnFormat::F30t
            | InsnFormat::F21t
            | InsnFormat::F22t
    )
}

fn branch_for(insn: &Instruction, level: Level) -> Option<Branch> {
    if !is_branch_format(insn_format(insn.op)) {
        return None;
    }
    let target = insn.target?;
    Some(match level {
        Level::Bc => Branch::Absolute(target),
        Level::Ar => {
            if target > insn.addr {
                Branch::Forward
            } else if target < insn.addr {
                Branch::Backward
            } else {
                Branch::Zero
            }
        }
    })
}

/// Resolve the pool reference to a stable string. Names survive at BC/AR
/// (they are erased at NR). Resolution failure on adversarial input
/// yields a placeholder rather than propagating an error — the token
/// stream is best-effort structural, never panicking.
fn pool_token(dex: &DexFile, insn: &Instruction) -> Option<String> {
    let pi = insn.pool_idx.as_ref()?;
    // The decoded IR routes both const-method-handle and const-method-type
    // through PoolIndex::Method, which is a mislabel (the operands are a
    // method-handle index and a proto index respectively). Key these off
    // the opcode so they are not resolved as method references.
    match insn.op {
        Opcode::ConstMethodHandle => return Some("<method-handle>".to_string()),
        Opcode::ConstMethodType => return Some("<method-type>".to_string()),
        _ => {}
    }
    Some(match pi {
        PoolIndex::String(s) => format!("\"{}\"", dex.get_string(*s).unwrap_or("<?str>")),
        PoolIndex::Type(t) => dex.get_type_descriptor(*t).unwrap_or("<?type>").to_string(),
        PoolIndex::Field(fld) => dex.format_field(*fld).unwrap_or_else(|_| "<?field>".to_string()),
        PoolIndex::Method(m) => dex.format_method(*m).unwrap_or_else(|_| "<?method>".to_string()),
        PoolIndex::MethodAndProto(m, _proto) => {
            dex.format_method(*m).unwrap_or_else(|_| "<?method>".to_string())
        }
        PoolIndex::CallSite(_) => "<call-site>".to_string(),
    })
}

fn token_for(dex: &DexFile, insn: &Instruction, level: Level) -> Token {
    let mut regs = Vec::new();
    if let Some(d) = insn.dst {
        regs.push(d);
    }
    regs.extend_from_slice(insn.src.as_slice());
    let lit = if format_has_immediate(insn_format(insn.op)) {
        Some(insn.literal)
    } else {
        None
    };
    Token {
        op: op_class(insn.op, level),
        regs,
        lit,
        branch: branch_for(insn, level),
        pool: pool_token(dex, insn),
    }
}

/// Encode one method's instruction stream at `level`.
///
/// Returns [`Unencodable::DecodeDesync`] if the method's decode
/// desynced on an unmapped opcode byte — no partial token sequence is
/// ever produced for a misaligned stream (honest-Unrecognized). At
/// `Ar`, NOPs are omitted.
pub fn encode_method(dex: &DexFile, code: &CodeItem, level: Level) -> Result<Vec<Token>, Unencodable> {
    if code
        .invariant_violations
        .iter()
        .any(|v| matches!(v, CodeItemInvariantViolation::UnknownOpcodeByte { .. }))
    {
        return Err(Unencodable::DecodeDesync);
    }
    let mut out = Vec::with_capacity(code.instructions.len());
    for insn in &code.instructions {
        if level == Level::Ar && insn.op == Opcode::Nop {
            continue;
        }
        out.push(token_for(dex, insn, level));
    }
    Ok(out)
}

/// Encode one method's instruction stream to a newline-joined token
/// string (the form a fuzzy hash will consume at the Fuzzy-SPR rung).
pub fn encode_method_string(
    dex: &DexFile,
    code: &CodeItem,
    level: Level,
) -> Result<String, Unencodable> {
    let toks = encode_method(dex, code, level)?;
    let mut s = String::new();
    for (i, t) in toks.iter().enumerate() {
        if i != 0 {
            s.push('\n');
        }
        s.push_str(&t.to_string());
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::RegList;
    use crate::parser::DexFile;

    fn fixture_dex() -> DexFile {
        let bytes = include_bytes!("../tests/fixtures/classes.dex");
        DexFile::parse(bytes, None).expect("fixture parses")
    }

    fn insn(op: Opcode, addr: u32, dst: Option<u16>, src: RegList, target: Option<u32>) -> Instruction {
        Instruction { addr, op, size: 1, dst, src, literal: 0, target, pool_idx: None }
    }

    #[test]
    fn ar_collapses_encoding_suffixes_bc_preserves_them() {
        // The AR rung must give the same op-class to every encoding of a
        // semantic operation; BC keeps them distinct. These are the
        // mandatory normalizations without which obfuscation invariance
        // fails (R8 re-selects encodings on register/index/value size).
        let pairs = [
            (Opcode::InvokeVirtual, Opcode::InvokeVirtualRange),
            (Opcode::ConstString, Opcode::ConstStringJumbo),
            (Opcode::Const4, Opcode::Const),
            (Opcode::Move, Opcode::MoveFrom16),
            (Opcode::Goto, Opcode::Goto32),
        ];
        for (a, b) in pairs {
            assert_eq!(
                op_class(a, Level::Ar),
                op_class(b, Level::Ar),
                "AR must collapse {a:?} and {b:?} to one class"
            );
            assert_ne!(
                op_class(a, Level::Bc),
                op_class(b, Level::Bc),
                "BC must keep {a:?} and {b:?} distinct"
            );
        }
        // And the collapsed class is the suffix-free semantic op.
        assert_eq!(op_class(Opcode::InvokeVirtualRange, Level::Ar), "invoke-virtual");
        assert_eq!(op_class(Opcode::ConstStringJumbo, Level::Ar), "const-string");
    }

    #[test]
    fn unknown_opcode_byte_method_is_unencodable() {
        // A desynced stream must bail, never tokenize a misaligned tail.
        let dex = fixture_dex();
        let code = CodeItem {
            registers_size: 1,
            ins_size: 0,
            outs_size: 0,
            debug_info_off: 0,
            instructions: vec![insn(Opcode::Return, 0, Some(0), RegList::empty(), None)],
            tries: Vec::new(),
            catch_handlers: Vec::new(),
            payloads: std::collections::BTreeMap::new(),
            invariant_violations: vec![CodeItemInvariantViolation::UnknownOpcodeByte {
                source_pc: 0,
                opcode_byte: 0xE3,
            }],
        };
        assert_eq!(encode_method(&dex, &code, Level::Ar), Err(Unencodable::DecodeDesync));
        assert_eq!(encode_method(&dex, &code, Level::Bc), Err(Unencodable::DecodeDesync));
    }

    #[test]
    fn ar_drops_nops_bc_keeps_them() {
        let dex = fixture_dex();
        let code = CodeItem {
            registers_size: 2,
            ins_size: 0,
            outs_size: 0,
            debug_info_off: 0,
            instructions: vec![
                insn(Opcode::Nop, 0, None, RegList::empty(), None),
                insn(Opcode::Move, 1, Some(0), RegList::one(1), None),
                insn(Opcode::Nop, 2, None, RegList::empty(), None),
                insn(Opcode::ReturnVoid, 3, None, RegList::empty(), None),
            ],
            tries: Vec::new(),
            catch_handlers: Vec::new(),
            payloads: std::collections::BTreeMap::new(),
            invariant_violations: Vec::new(),
        };
        let bc = encode_method(&dex, &code, Level::Bc).expect("bc");
        let ar = encode_method(&dex, &code, Level::Ar).expect("ar");
        assert_eq!(bc.len(), 4, "BC retains NOPs");
        assert_eq!(ar.len(), 2, "AR drops both NOPs");
        assert!(ar.iter().all(|t| t.op != "nop"));
    }

    #[test]
    fn branch_is_directional_at_ar_absolute_at_bc() {
        let dex = fixture_dex();
        // A backward goto at addr 5 targeting addr 2.
        let back = insn(Opcode::Goto, 5, None, RegList::empty(), Some(2));
        // A forward goto at addr 5 targeting addr 9.
        let fwd = insn(Opcode::Goto, 5, None, RegList::empty(), Some(9));
        assert_eq!(token_for(&dex, &back, Level::Ar).branch, Some(Branch::Backward));
        assert_eq!(token_for(&dex, &fwd, Level::Ar).branch, Some(Branch::Forward));
        assert_eq!(token_for(&dex, &back, Level::Bc).branch, Some(Branch::Absolute(2)));
        // A non-branch op carries no branch even when target is set
        // (switch/fill use target for a payload pc, not a branch).
        let sw = insn(Opcode::PackedSwitch, 5, Some(0), RegList::empty(), Some(20));
        assert_eq!(token_for(&dex, &sw, Level::Ar).branch, None);
    }

    /// Route-A invariance gate (structural skeleton): renaming every type
    /// descriptor must change resolved pool references but leave the
    /// op-class / register / literal / branch skeleton byte-identical —
    /// the property NR then completes by erasing the names entirely.
    #[test]
    fn structural_skeleton_is_independent_of_type_names() {
        let mut dex = fixture_dex();

        let skeleton = |d: &DexFile| -> Vec<Vec<Token>> {
            d.code_items
                .values()
                .map(|c| {
                    encode_method(d, c, Level::Ar)
                        .expect("fixture methods encode")
                        .into_iter()
                        .map(|mut t| {
                            t.pool = None; // project away names
                            t
                        })
                        .collect()
                })
                .collect()
        };
        let full = |d: &DexFile| -> Vec<String> {
            d.code_items
                .values()
                .map(|c| encode_method_string(d, c, Level::Ar).expect("encodes"))
                .collect()
        };

        let before_skeleton = skeleton(&dex);
        let before_full = full(&dex);

        // Synthetic rename: prepend a marker into every class descriptor
        // (Lcom/x/Foo; -> Lren_/com/x/Foo;). Structure is untouched.
        for d in dex.type_descriptors.iter_mut() {
            if let Some(rest) = d.strip_prefix('L') {
                *d = format!("Lren_/{rest}");
            }
        }

        let after_skeleton = skeleton(&dex);
        let after_full = full(&dex);

        assert_eq!(
            before_skeleton, after_skeleton,
            "AR structural skeleton must be independent of type names"
        );
        assert_ne!(
            before_full, after_full,
            "the rename must actually have moved some resolved pool reference \
             (else the test is vacuous)"
        );
    }
}
