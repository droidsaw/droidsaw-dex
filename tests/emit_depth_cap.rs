//! Emit-stmt recursion depth cap: a handcrafted `Stmt` tree of depth >
//! `emit::MAX_STMT_DEPTH` returns a typed err via
//! `EmitCtx::error_state` instead of blowing the stack.
//!
//! Tests two conditions:
//!   - Positive: depth ≪ cap emits cleanly (half-cap smoke).
//!   - Negative: depth just past cap records
//!     `DexError::EmitRecursionDepthExceeded` and emits an empty-on-sentinel
//!     outer String.
//!
//! Mirrors `droidsaw-common`'s `region-recursion-depth-cap` pattern.
//! Lives as an integration test to exercise the real `emit_stmt` path.

use droidsaw_dex::emit::{emit_stmt, EmitCtx, MAX_STMT_DEPTH};
use droidsaw_dex::error::DexError;
use droidsaw_dex::parser::DexFile;
use droidsaw_dex::structure::{Condition, Stmt};
use droidsaw_dex::ssa::VarId;
use droidsaw_dex::types::TypeEnv;

/// Build a nested `Stmt::If` chain of exactly `n` depth levels. Each
/// level wraps the child in `If { cond: TestZero(v0_0), then: child,
/// else: None }`. The terminal level is a bare `Stmt::Return(None)`.
fn nested_if_chain(n: usize) -> Stmt {
    let v = VarId::new(0, 0);
    let mut stmt: Stmt = Stmt::Return(None);
    for _ in 0..n {
        stmt = Stmt::If {
            cond: Condition::TestZero {
                var: v.clone(),
                op: droidsaw_dex::opcodes::Opcode::IfEqz,
            },
            then_body: Box::new(stmt),
            else_body: None,
        };
    }
    stmt
}

fn minimal_dex_and_env() -> (Vec<u8>, DexFile, TypeEnv) {
    // Reuse the Minimal fixture — the depth cap doesn't care about
    // the method body; we synthesize the Stmt tree ourselves.
    let data: &[u8] = include_bytes!("fixtures/classes.dex");
    let dex = DexFile::parse(data, None).expect("parse Minimal fixture");
    let env = TypeEnv {
        types: rustc_hash::FxHashMap::default(),
        casts: Vec::new(),
    };
    (data.to_vec(), dex, env)
}

/// Unwind a nested-If chain iteratively so Rust's auto-generated
/// recursive `Drop` doesn't blow the stack on multi-thousand-deep
/// trees. Runs after the emit-cap assertions have captured their
/// result. Without this, the test thread panics on cleanup even
/// though the depth cap itself fired correctly.
fn unwind_nested_if(mut stmt: Stmt) {
    while let Stmt::If { then_body, .. } = stmt {
        stmt = *then_body;
    }
}

/// `emit_stmt_depth`'s per-frame footprint (big match + `String`
/// locals) is ~8 KB, so recursing to `MAX_STMT_DEPTH = 512` needs
/// ~4 MB of stack. The default Rust test thread is 2 MB — too small.
/// Each test runs on a spawned thread with a 16 MB stack (matches
/// `common::region`'s test convention) so the recursion can reach
/// the cap without tripping thread-stack before the cap check fires.
/// Production paths run on the main thread which has an 8 MB stack
/// by default on Linux; `MAX_STMT_DEPTH = 512` lands well within
/// that budget (~4 MB used at saturation).
const TEST_THREAD_STACK: usize = 16 * 1024 * 1024;

fn spawn_with_large_stack<F: FnOnce() + Send + 'static>(f: F) {
    let h = std::thread::Builder::new()
        .stack_size(TEST_THREAD_STACK)
        .spawn(f)
        .expect("spawn large-stack test thread");
    h.join().expect("large-stack test thread panicked");
}

#[test]
fn depth_half_cap_emits_cleanly() {
    spawn_with_large_stack(|| {
        let (_data, dex, env) = minimal_dex_and_env();
        let stmt = nested_if_chain(MAX_STMT_DEPTH / 2);
        let mut ctx = EmitCtx::new();
        let out = emit_stmt(&stmt, &env, &dex, 0, &mut ctx);
        let err = ctx.error_state.is_none();
        let non_empty = !out.is_empty();
        drop(out);
        unwind_nested_if(stmt);
        assert!(err, "half-cap depth should not record overflow");
        assert!(non_empty, "half-cap emit returned empty-on-sentinel unexpectedly");
    });
}

#[test]
fn depth_past_cap_records_typed_err() {
    spawn_with_large_stack(|| {
        let (_data, dex, env) = minimal_dex_and_env();
        let stmt = nested_if_chain(MAX_STMT_DEPTH + 1);
        let mut ctx = EmitCtx::new();
        let _ = emit_stmt(&stmt, &env, &dex, 0, &mut ctx);
        let recorded = ctx.error_state.take();
        unwind_nested_if(stmt);
        match recorded {
            Some(DexError::EmitRecursionDepthExceeded { depth, cap }) => {
                assert_eq!(cap, MAX_STMT_DEPTH);
                assert!(depth > cap, "recorded depth {depth} should exceed cap {cap}");
            }
            other => panic!("expected EmitRecursionDepthExceeded; got {other:?}"),
        }
    });
}
