//! Synthetic-IR unit coverage for `signatures::matchers`.
//!
//! Doctests cover the happy paths inline; this integration suite focuses
//! on edge cases (empty seq, single-stmt seq, length mismatch, var
//! aliasing, throw vs same-shape Throw on different var). DexFile-pool
//! walking matchers (`matches_invoke_named`, `matches_invoke_signature`,
//! `matches_new_instance_of`, `matches_const_string`) are exercised via
//! `tests/corpus_check.rs` + `tests/fixture_ratchet.rs` against real
//! fixtures; constructing a synthetic `DexFile` for unit-level coverage
//! reaches into the parser surface area and is out of scope for the
//! matcher suite per the precedent set by
//! `signatures/javac21/string_concat_indy.rs::tests` which routes
//! pool-walk coverage to the corpus.
#![allow(clippy::unwrap_used, clippy::expect_used)] // WHY: test-only

use droidsaw_dex::decode::{Instruction, RegList};
use droidsaw_dex::opcodes::Opcode;
use droidsaw_dex::signatures::matchers::{
    as_switch_at, body_writes_var, matches_assign, matches_if_else, matches_seq,
    matches_seq_prefix, matches_throw_var, var_eq, var_used_in,
};
use droidsaw_dex::ssa::{SsaInsn, VarId};
use droidsaw_dex::structure::{CatchClause, Condition, Stmt};

// ── Fixture builders ────────────────────────────────────────────────

fn nop_insn(dst: Option<VarId>, uses: Vec<VarId>) -> SsaInsn {
    SsaInsn {
        insn: Instruction {
            addr: 0,
            op: Opcode::Nop,
            size: 0,
            dst: None,
            src: RegList::empty(),
            literal: 0,
            target: None,
            pool_idx: None,
        },
        dst,
        uses,
    }
}

fn const_assign(dst: VarId) -> Stmt {
    Stmt::Expr(SsaInsn {
        insn: Instruction {
            addr: 0,
            op: Opcode::Const,
            size: 0,
            dst: None,
            src: RegList::empty(),
            literal: 0,
            target: None,
            pool_idx: None,
        },
        dst: Some(dst),
        uses: vec![],
    })
}

// ── as_switch_at ────────────────────────────────────────────────────

#[test]
fn as_switch_at_extracts_value_cases_default() {
    let v = VarId::new(3, 1);
    let stmts = vec![Stmt::Switch {
        value: v.clone(),
        cases: vec![
            (vec![1, 2], Box::new(Stmt::Break)),
            (vec![3], Box::new(Stmt::Continue)),
        ],
        default: Some(Box::new(Stmt::Break)),
    }];
    let (val, cases, default) = as_switch_at(&stmts, 0).expect("non-empty switch");
    assert_eq!(*val, v);
    assert_eq!(cases.len(), 2);
    assert!(default.is_some());
}

#[test]
fn as_switch_at_rejects_empty_cases() {
    let stmts = vec![Stmt::Switch {
        value: VarId::new(0, 0),
        cases: vec![],
        default: None,
    }];
    assert!(as_switch_at(&stmts, 0).is_none());
}

#[test]
fn as_switch_at_rejects_out_of_bounds_position() {
    let stmts = vec![Stmt::Break];
    assert!(as_switch_at(&stmts, 1).is_none());
    assert!(as_switch_at(&stmts, 99).is_none());
    assert!(as_switch_at(&[], 0).is_none());
}

#[test]
fn as_switch_at_rejects_non_switch() {
    let stmts = vec![Stmt::Break, Stmt::Continue];
    assert!(as_switch_at(&stmts, 0).is_none());
    assert!(as_switch_at(&stmts, 1).is_none());
}

// ── matches_seq / matches_seq_prefix ────────────────────────────────

#[test]
fn matches_seq_empty_body() {
    fn never(_: &Stmt) -> bool {
        false
    }
    let body = Stmt::Seq(vec![]);
    assert!(matches_seq(&body, &[]));
    assert!(!matches_seq(&body, &[never]));
}

#[test]
fn matches_seq_single_stmt() {
    fn is_break(s: &Stmt) -> bool {
        matches!(s, Stmt::Break)
    }
    let body = Stmt::Seq(vec![Stmt::Break]);
    assert!(matches_seq(&body, &[is_break]));
}

#[test]
fn matches_seq_length_mismatch_rejects() {
    fn is_break(s: &Stmt) -> bool {
        matches!(s, Stmt::Break)
    }
    let body = Stmt::Seq(vec![Stmt::Break, Stmt::Continue]);
    assert!(!matches_seq(&body, &[is_break]));
    let body2 = Stmt::Seq(vec![Stmt::Break]);
    assert!(!matches_seq(&body2, &[is_break, is_break]));
}

#[test]
fn matches_seq_non_seq_rejects() {
    fn always(_: &Stmt) -> bool {
        true
    }
    assert!(!matches_seq(&Stmt::Break, &[]));
    assert!(!matches_seq(&Stmt::Break, &[always]));
}

#[test]
fn matches_seq_prefix_accepts_trailing_extras() {
    fn is_break(s: &Stmt) -> bool {
        matches!(s, Stmt::Break)
    }
    let body = Stmt::Seq(vec![Stmt::Break, Stmt::Break, Stmt::Continue, Stmt::Break]);
    assert!(matches_seq_prefix(&body, &[is_break, is_break]));
    assert!(matches_seq_prefix(&body, &[is_break]));
    // Empty matchers always match (vacuous prefix).
    assert!(matches_seq_prefix(&body, &[]));
}

#[test]
fn matches_seq_prefix_rejects_too_short_body() {
    fn always(_: &Stmt) -> bool {
        true
    }
    let body = Stmt::Seq(vec![Stmt::Break]);
    assert!(!matches_seq_prefix(&body, &[always, always]));
}

// ── matches_assign ──────────────────────────────────────────────────

#[test]
fn matches_assign_dst_aliasing() {
    fn always(_: &SsaInsn) -> bool {
        true
    }
    let v0 = VarId::new(0, 0);
    let v0_v1 = VarId::new(0, 1); // same reg, different SSA version → DIFFERENT VarId
    let stmt = const_assign(v0.clone());
    assert!(matches_assign(&stmt, &v0, always));
    assert!(!matches_assign(&stmt, &v0_v1, always));
}

#[test]
fn matches_assign_inner_predicate_rejection() {
    fn never(_: &SsaInsn) -> bool {
        false
    }
    fn always(_: &SsaInsn) -> bool {
        true
    }
    let v = VarId::new(0, 0);
    let stmt = const_assign(v.clone());
    assert!(!matches_assign(&stmt, &v, never));
    assert!(matches_assign(&stmt, &v, always));
}

#[test]
fn matches_assign_non_expr_stmt_rejects() {
    fn always(_: &SsaInsn) -> bool {
        true
    }
    assert!(!matches_assign(&Stmt::Break, &VarId::new(0, 0), always));
    assert!(!matches_assign(
        &Stmt::Throw(VarId::new(0, 0)),
        &VarId::new(0, 0),
        always
    ));
}

// ── matches_throw_var ───────────────────────────────────────────────

#[test]
fn matches_throw_var_ssa_version_distinguishes() {
    let v0 = VarId::new(0, 0);
    let v0_v1 = VarId::new(0, 1);
    assert!(matches_throw_var(&Stmt::Throw(v0.clone()), &v0));
    // Same register, different SSA version — different VarId.
    assert!(!matches_throw_var(&Stmt::Throw(v0_v1.clone()), &v0));
}

#[test]
fn matches_throw_var_rejects_other_stmts() {
    let v = VarId::new(0, 0);
    assert!(!matches_throw_var(&Stmt::Break, &v));
    assert!(!matches_throw_var(&Stmt::Return(Some(v.clone())), &v));
}

// ── matches_if_else ─────────────────────────────────────────────────

#[test]
fn matches_if_else_requires_else_branch() {
    let v = VarId::new(0, 0);
    fn always(_: &Stmt) -> bool {
        true
    }
    fn always_cond(_: &Condition) -> bool {
        true
    }
    let bare = Stmt::If {
        cond: Condition::Var(v.clone()),
        then_body: Box::new(Stmt::Break),
        else_body: None,
    };
    assert!(!matches_if_else(&bare, always_cond, always, always));

    let with_else = Stmt::If {
        cond: Condition::Var(v),
        then_body: Box::new(Stmt::Break),
        else_body: Some(Box::new(Stmt::Continue)),
    };
    assert!(matches_if_else(&with_else, always_cond, always, always));
}

#[test]
fn matches_if_else_threads_inner_predicates() {
    let v = VarId::new(0, 0);
    fn cond_var(c: &Condition) -> bool {
        matches!(c, Condition::Var(_))
    }
    fn is_break(s: &Stmt) -> bool {
        matches!(s, Stmt::Break)
    }
    fn is_continue(s: &Stmt) -> bool {
        matches!(s, Stmt::Continue)
    }
    let stmt = Stmt::If {
        cond: Condition::Var(v),
        then_body: Box::new(Stmt::Break),
        else_body: Some(Box::new(Stmt::Continue)),
    };
    assert!(matches_if_else(&stmt, cond_var, is_break, is_continue));
    // Wrong then/else assignment — should reject.
    assert!(!matches_if_else(&stmt, cond_var, is_continue, is_break));
}

// ── var_eq / var_used_in ────────────────────────────────────────────

#[test]
fn var_eq_distinguishes_register_and_version() {
    assert!(var_eq(&VarId::new(0, 0), &VarId::new(0, 0)));
    assert!(!var_eq(&VarId::new(0, 0), &VarId::new(1, 0)));
    // SSA version distinguishes.
    assert!(!var_eq(&VarId::new(0, 0), &VarId::new(0, 1)));
}

#[test]
fn var_used_in_walks_all_uses() {
    let v0 = VarId::new(0, 0);
    let v1 = VarId::new(1, 0);
    let v2 = VarId::new(2, 0);
    let insn = nop_insn(None, vec![v0.clone(), v1.clone()]);
    assert!(var_used_in(&v0, &insn));
    assert!(var_used_in(&v1, &insn));
    assert!(!var_used_in(&v2, &insn));
}

#[test]
fn var_used_in_empty_uses() {
    let v = VarId::new(0, 0);
    let insn = nop_insn(None, vec![]);
    assert!(!var_used_in(&v, &insn));
}

// ── body_writes_var ─────────────────────────────────────────────────

#[test]
fn body_writes_var_finds_assign_in_seq() {
    let v = VarId::new(0, 0);
    let body = Stmt::Seq(vec![Stmt::Break, const_assign(v.clone()), Stmt::Continue]);
    assert!(body_writes_var(&body, &v));
}

#[test]
fn body_writes_var_walks_into_if_branches() {
    let v = VarId::new(2, 1);
    let then_branch = Stmt::Seq(vec![const_assign(v.clone())]);
    let stmt = Stmt::If {
        cond: Condition::Var(VarId::new(0, 0)),
        then_body: Box::new(then_branch),
        else_body: Some(Box::new(Stmt::Break)),
    };
    assert!(body_writes_var(&stmt, &v));
}

#[test]
fn body_writes_var_walks_into_else_branch() {
    let v = VarId::new(5, 0);
    let stmt = Stmt::If {
        cond: Condition::Var(VarId::new(0, 0)),
        then_body: Box::new(Stmt::Break),
        else_body: Some(Box::new(const_assign(v.clone()))),
    };
    assert!(body_writes_var(&stmt, &v));
}

#[test]
fn body_writes_var_walks_into_try_and_catches() {
    let v_try = VarId::new(7, 0);
    let v_catch = VarId::new(8, 0);
    let stmt = Stmt::TryCatch {
        body: Box::new(const_assign(v_try.clone())),
        catches: vec![CatchClause {
            exception_type: None,
            var: Some(VarId::new(99, 0)),
            body: Stmt::Seq(vec![const_assign(v_catch.clone())]),
        }],
    };
    assert!(body_writes_var(&stmt, &v_try));
    assert!(body_writes_var(&stmt, &v_catch));
    assert!(!body_writes_var(&stmt, &VarId::new(0, 0)));
}

#[test]
fn body_writes_var_does_not_descend_into_switch_arms() {
    // Per matcher rustdoc: Switch/MultiArm bodies are out-of-region by
    // recognizer convention. body_writes_var stops at the Switch node.
    let v = VarId::new(4, 0);
    let stmt = Stmt::Switch {
        value: VarId::new(0, 0),
        cases: vec![(vec![1], Box::new(const_assign(v.clone())))],
        default: None,
    };
    assert!(!body_writes_var(&stmt, &v));
}

#[test]
fn body_writes_var_unrelated_var_returns_false() {
    let v = VarId::new(0, 0);
    let other = VarId::new(99, 99);
    let body = Stmt::Seq(vec![const_assign(v)]);
    assert!(!body_writes_var(&body, &other));
}
