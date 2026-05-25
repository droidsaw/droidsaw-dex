// ADAPTER-PROPERTY: ReversedCfg::successors(n) == predecessors of n
// in forward CFG (transpose invariant). See structure.rs:454 PROOF block
// for mint-discipline rationale. Tests both directions; covers virtual_exit sentinel.
//
// This module verifies that `droidsaw_dex::structure::ReversedCfg` (the
// adapter used by `compute_post_dominators`) is a correct transpose of the
// forward CFG's normal-flow view.
//
// For every real block `n` in a forward CFG `G`:
//   reversed.successors(n) == { p ∈ G.nodes() : n ∈ normal_flow_succs(p) }
//   reversed.predecessors(n) == normal_flow_succs(n)
//
// For the `virtual_exit` sentinel:
//   reversed.successors(virtual_exit) == non-empty blocks with no normal-flow
//     outgoing edges (the method terminal set)
//   reversed.predecessors(virtual_exit) == empty

use std::collections::{BTreeMap, BTreeSet};

use droidsaw_dex::cfg::{BasicBlock, BlockIdx, Cfg, Edge, EdgeKind};
use droidsaw_dex::decode::{Instruction, RegList};
use droidsaw_dex::opcodes::Opcode;
use droidsaw_dex::structure::ReversedCfg;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// A minimal, structurally-valid `Instruction` (ReturnVoid) for use in tests
/// that need a non-empty `instructions` vec.  We only care that the vec is
/// non-empty; the opcode semantics do not matter here.
fn ret_void_insn() -> Instruction {
    Instruction {
        addr: 0,
        op: Opcode::ReturnVoid,
        size: 1,
        dst: None,
        src: RegList::empty(),
        literal: 0,
        target: None,
        pool_idx: None,
    }
}

/// Build a `BasicBlock` with the given id and successors; `instructions` is
/// empty by default (consistent with the `virtual_exit` exclusion rule).
/// Fill `predecessors` via `fill_predecessors_for_blocks` after all blocks
/// are assembled.
fn block(id: u32, normal_succs: &[u32]) -> BasicBlock {
    BasicBlock {
        id: BlockIdx(id),
        start_addr: id * 4,
        instructions: Vec::new(),
        successors: normal_succs
            .iter()
            .map(|&t| Edge {
                target: BlockIdx(t),
                kind: EdgeKind::FallThrough,
            })
            .collect(),
        predecessors: BTreeSet::new(),
    }
}

/// Like `block`, but mark the block as having instructions so that the
/// `virtual_exit` rule fires when there are no normal-flow successors.
fn block_with_insns(id: u32, normal_succs: &[u32]) -> BasicBlock {
    let mut b = block(id, normal_succs);
    b.instructions.push(ret_void_insn());
    b
}

/// Fill the `predecessors` set for each block from the `successors` lists.
/// This is the same logic as `cfg::fill_predecessors`.
fn fill_predecessors(blocks: &mut [BasicBlock]) {
    let edges: Vec<(BlockIdx, BlockIdx)> = blocks
        .iter()
        .flat_map(|b| b.successors.iter().map(move |e| (b.id, e.target)))
        .collect();
    for (src, dst) in edges {
        blocks[dst.0 as usize].predecessors.insert(src);
    }
}

/// Build a `Cfg` from a list of blocks (already `fill_predecessors`-d).
fn build_cfg(blocks: Vec<BasicBlock>) -> Cfg {
    let addr_to_block = blocks
        .iter()
        .map(|b| (b.start_addr, b.id))
        .collect::<BTreeMap<_, _>>();
    Cfg {
        entry: BlockIdx(0),
        blocks,
        exception_regions: Vec::new(),
        addr_to_block,
    }
}

// ── Core property checker ─────────────────────────────────────────────────────

/// Assert the full transpose invariant for the given `Cfg`.
///
/// For every real block `n`:
///   reversed.successors(n) == { p : n ∈ normal_flow_succs(p) }
///   reversed.predecessors(n) == normal_flow_succs(n) ∪ { virtual_exit if n is a terminal }
///
/// The second formula captures the virtual_exit entry node: since virtual_exit has
/// edges to all non-empty terminal blocks (blocks with no normal-flow outgoing edges
/// but with instructions), each such terminal block has virtual_exit as a predecessor
/// in the reversed graph.
///
/// For the virtual_exit sentinel:
///   reversed.successors(virtual_exit) == non-empty blocks with no normal-flow succs
///   reversed.predecessors(virtual_exit) == empty
fn assert_transpose_invariant(cfg: &Cfg) {
    if cfg.blocks.is_empty() {
        // Empty CFG — nothing to check.
        return;
    }

    let virtual_exit = BlockIdx(cfg.blocks.len() as u32);
    let reversed = ReversedCfg { cfg, virtual_exit };

    // Build ground-truth: normal-flow successors for each block (excluding
    // exception edges).
    let normal_succs: BTreeMap<BlockIdx, BTreeSet<BlockIdx>> = cfg
        .blocks
        .iter()
        .map(|b| {
            let succs: BTreeSet<BlockIdx> = b
                .successors
                .iter()
                .filter(|e| {
                    !matches!(
                        e.kind,
                        EdgeKind::ExceptionHandler(_) | EdgeKind::ExceptionCatchAll
                    )
                })
                .map(|e| e.target)
                .collect();
            (b.id, succs)
        })
        .collect();

    // For each real block `n`, verify both transpose directions.
    for n in cfg.blocks.iter().map(|b| b.id) {
        // Direction 1: reversed.successors(n) == { p : n ∈ normal_succs(p) }
        let expected_rev_succ: BTreeSet<BlockIdx> = cfg
            .blocks
            .iter()
            .filter_map(|p| {
                if normal_succs[&p.id].contains(&n) {
                    Some(p.id)
                } else {
                    None
                }
            })
            .collect();
        let got_rev_succ: BTreeSet<BlockIdx> = reversed.successors_sorted(n);
        assert_eq!(
            got_rev_succ,
            expected_rev_succ,
            "reversed.successors({n:?}) failed transpose invariant (dir-1: rev_succ == preds-in-fwd)",
        );

        // Direction 2: reversed.predecessors(n) == normal_succs(n) ∪ { virtual_exit if n is terminal }
        //
        // In the reversed graph, virtual_exit is the entry node and has edges to all non-empty
        // terminal blocks (those with no normal-flow successors but with instructions).  From the
        // perspective of a terminal block `n`, virtual_exit is therefore a *predecessor* in the
        // reversed graph.  The formula extends the simple transpose:
        //   reversed.predecessors(n) == fwd_succs(n)  [+ virtual_exit if n is a terminal]
        let mut expected_rev_pred: BTreeSet<BlockIdx> = normal_succs[&n].clone();
        let is_terminal = {
            let b = &cfg.blocks[n.0 as usize];
            let has_normal_succ = b.successors.iter().any(|e| {
                !matches!(e.kind, EdgeKind::ExceptionHandler(_) | EdgeKind::ExceptionCatchAll)
            });
            !has_normal_succ && !b.instructions.is_empty()
        };
        if is_terminal {
            expected_rev_pred.insert(virtual_exit);
        }
        let got_rev_pred: BTreeSet<BlockIdx> = reversed.predecessors_sorted(n);
        assert_eq!(
            got_rev_pred,
            expected_rev_pred,
            "reversed.predecessors({n:?}) failed transpose invariant \
             (dir-2: rev_pred == fwd_succs [+ virtual_exit if terminal])",
        );
    }

    // Virtual-exit direction: successors should be non-empty terminal blocks.
    let expected_terminals: BTreeSet<BlockIdx> = cfg
        .blocks
        .iter()
        .filter(|b| {
            let has_normal_succ = b.successors.iter().any(|e| {
                !matches!(
                    e.kind,
                    EdgeKind::ExceptionHandler(_) | EdgeKind::ExceptionCatchAll
                )
            });
            !has_normal_succ && !b.instructions.is_empty()
        })
        .map(|b| b.id)
        .collect();
    let got_ve_succ: BTreeSet<BlockIdx> = reversed.successors_sorted(virtual_exit);
    assert_eq!(
        got_ve_succ,
        expected_terminals,
        "reversed.successors(virtual_exit) should equal non-empty terminal blocks",
    );

    // Virtual-exit predecessors should be empty.
    let got_ve_pred: BTreeSet<BlockIdx> = reversed.predecessors_sorted(virtual_exit);
    assert!(
        got_ve_pred.is_empty(),
        "reversed.predecessors(virtual_exit) should be empty, got {got_ve_pred:?}",
    );
}

/// Helper trait to make `ReversedCfg` usable in the property checker without
/// importing `droidsaw_common::graph::Graph` in tests.
///
/// Both methods collect the `Vec` result from the `Graph` impl into a
/// `BTreeSet` so comparisons are order-independent.
trait ReversedCfgExt {
    fn successors_sorted(&self, n: BlockIdx) -> BTreeSet<BlockIdx>;
    fn predecessors_sorted(&self, n: BlockIdx) -> BTreeSet<BlockIdx>;
}

impl<'a> ReversedCfgExt for ReversedCfg<'a> {
    fn successors_sorted(&self, n: BlockIdx) -> BTreeSet<BlockIdx> {
        use droidsaw_common::graph::Graph as _;
        self.successors(n).into_iter().collect()
    }

    fn predecessors_sorted(&self, n: BlockIdx) -> BTreeSet<BlockIdx> {
        use droidsaw_common::graph::Graph as _;
        self.predecessors(n).into_iter().collect()
    }
}

// ── Shape 1: linear chain ────────────────────────────────────────────────────

/// A → B → C (all normal-flow).
/// Terminal: C (no succs, has insns).
#[test]
fn transpose_linear_chain() {
    let mut blocks = vec![
        block_with_insns(0, &[1]), // A → B
        block_with_insns(1, &[2]), // B → C
        block_with_insns(2, &[]),  // C (terminal)
    ];
    fill_predecessors(&mut blocks);
    let cfg = build_cfg(blocks);
    assert_transpose_invariant(&cfg);
}

// ── Shape 2: diamond ─────────────────────────────────────────────────────────

/// A → {B, C}; B → D; C → D.
/// Terminal: D.
#[test]
fn transpose_diamond() {
    let mut blocks = vec![
        block_with_insns(0, &[1, 2]), // A → B, C
        block_with_insns(1, &[3]),    // B → D
        block_with_insns(2, &[3]),    // C → D
        block_with_insns(3, &[]),     // D (terminal)
    ];
    fill_predecessors(&mut blocks);
    let cfg = build_cfg(blocks);
    assert_transpose_invariant(&cfg);
}

// ── Shape 3: natural loop ────────────────────────────────────────────────────

/// A → B (loop header); B → {C, exit(D)}; C → B (back-edge); D terminal.
#[test]
fn transpose_natural_loop() {
    let mut blocks = vec![
        block_with_insns(0, &[1]),    // A → B
        block_with_insns(1, &[2, 3]), // B → C (body) or D (exit)
        block_with_insns(2, &[1]),    // C → B (back-edge)
        block_with_insns(3, &[]),     // D (terminal)
    ];
    fill_predecessors(&mut blocks);
    let cfg = build_cfg(blocks);
    assert_transpose_invariant(&cfg);
}

// ── Shape 4: switch / fan-out ────────────────────────────────────────────────

/// A → {B, C, D, E} (switch); B, C, D, E all → F (merge); F terminal.
#[test]
fn transpose_switch_fan_out() {
    let mut blocks = vec![
        block_with_insns(0, &[1, 2, 3, 4]), // switch
        block_with_insns(1, &[5]),           // arm B
        block_with_insns(2, &[5]),           // arm C
        block_with_insns(3, &[5]),           // arm D
        block_with_insns(4, &[5]),           // arm E
        block_with_insns(5, &[]),            // merge F (terminal)
    ];
    fill_predecessors(&mut blocks);
    let cfg = build_cfg(blocks);
    assert_transpose_invariant(&cfg);
}

// ── Shape 5: try-catch with exception edges ──────────────────────────────────

/// A → B (normal); A → H (exception edge, excluded from normal-flow).
/// B → (terminal); H → (terminal).
/// The property must hold on the *normal-flow* projection only.
#[test]
fn transpose_try_catch_exception_edges() {
    // Block 0 (A): normal successor B(1); exception edge → H(2).
    let block_a = BasicBlock {
        id: BlockIdx(0),
        start_addr: 0,
        instructions: vec![ret_void_insn()],
        successors: vec![
            Edge {
                target: BlockIdx(1),
                kind: EdgeKind::FallThrough,
            },
            Edge {
                target: BlockIdx(2),
                kind: EdgeKind::ExceptionHandler(droidsaw_dex::ids::TypeIdx(0)),
            },
        ],
        predecessors: BTreeSet::new(),
    };
    // Block 1 (B): no successors (terminal).
    let block_b = block_with_insns(1, &[]);
    // Block 2 (H): exception handler, no successors (terminal).
    let block_h = block_with_insns(2, &[]);

    let mut blocks = vec![block_a, block_b, block_h];
    fill_predecessors(&mut blocks);
    let cfg = build_cfg(blocks);
    assert_transpose_invariant(&cfg);
}

// ── Shape 6: multiple terminal blocks (multi-return) ─────────────────────────

/// A → {B, C}; B terminal; C terminal.
/// virtual_exit should point to both B and C.
#[test]
fn transpose_multi_terminal_virtual_exit() {
    let mut blocks = vec![
        block_with_insns(0, &[1, 2]), // A → B, C
        block_with_insns(1, &[]),     // B (terminal)
        block_with_insns(2, &[]),     // C (terminal)
    ];
    fill_predecessors(&mut blocks);
    let cfg = build_cfg(blocks);

    let virtual_exit = BlockIdx(cfg.blocks.len() as u32);
    let reversed = ReversedCfg { cfg: &cfg, virtual_exit };

    // Both B and C should appear in virtual_exit's successors.
    let ve_succs: BTreeSet<BlockIdx> = reversed.successors_sorted(virtual_exit);
    assert!(
        ve_succs.contains(&BlockIdx(1)),
        "virtual_exit.successors should contain B(1); got {ve_succs:?}",
    );
    assert!(
        ve_succs.contains(&BlockIdx(2)),
        "virtual_exit.successors should contain C(2); got {ve_succs:?}",
    );

    // Full transpose invariant.
    assert_transpose_invariant(&cfg);
}

// ── Shape 7: empty blocks excluded from virtual_exit ─────────────────────────

/// A(with_insns) → B(no insns/empty terminal).
/// B has no normal-flow successors and no instructions.
/// virtual_exit should NOT point to B (empty block exclusion rule).
#[test]
fn transpose_empty_block_excluded_from_virtual_exit() {
    let mut blocks = vec![
        block_with_insns(0, &[1]), // A → B
        block(1, &[]),             // B: no insns, no successors
    ];
    fill_predecessors(&mut blocks);
    let cfg = build_cfg(blocks);

    let virtual_exit = BlockIdx(cfg.blocks.len() as u32);
    let reversed = ReversedCfg { cfg: &cfg, virtual_exit };

    let ve_succs: BTreeSet<BlockIdx> = reversed.successors_sorted(virtual_exit);
    assert!(
        !ve_succs.contains(&BlockIdx(1)),
        "empty block B(1) should NOT be in virtual_exit.successors; got {ve_succs:?}",
    );

    // Full transpose invariant.
    assert_transpose_invariant(&cfg);
}

// ── Shape 8: exception-only terminal excluded from normal-flow ────────────────

/// A → B (normal-flow); A → H (exception edge only, H has no normal successors).
/// H is non-empty but its only edge is the exception edge.
/// Exception edges are excluded from the normal-flow transpose.
#[test]
fn transpose_exc_only_terminal_excluded() {
    // Block 0 (A): normal FallThrough → B(1); exception → H(2).
    let block_a = BasicBlock {
        id: BlockIdx(0),
        start_addr: 0,
        instructions: vec![ret_void_insn()],
        successors: vec![
            Edge {
                target: BlockIdx(1),
                kind: EdgeKind::FallThrough,
            },
            Edge {
                target: BlockIdx(2),
                kind: EdgeKind::ExceptionHandler(droidsaw_dex::ids::TypeIdx(0)),
            },
        ],
        predecessors: BTreeSet::new(),
    };
    // Block 1 (B): normal terminal.
    let block_b = block_with_insns(1, &[]);
    // Block 2 (H): has insns, only exception-sourced (no normal-flow preds).
    // H has no outgoing normal edges either.
    let block_h = block_with_insns(2, &[]);

    let mut blocks = vec![block_a, block_b, block_h];
    fill_predecessors(&mut blocks);
    let cfg = build_cfg(blocks);
    assert_transpose_invariant(&cfg);
}

// ── Real-corpus integration ───────────────────────────────────────────────────

/// Walk all DEX files under `tests/fixtures/`, parse each one, build a CFG
/// for every method, and assert the transpose invariant on the resulting
/// `ReversedCfg`.
///
/// This exercises the property on real-world bytecode shapes that the
/// hand-enumerated tests above may not cover (e.g., sparse switch, complex
/// loop nests, multi-catch regions).
#[test]
fn transpose_real_corpus_fixtures() {
    use droidsaw_dex::decode::{parse_class_data, parse_code_item};
    use droidsaw_dex::DexFile;

    let fixture_dir = std::path::Path::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"),
    );

    let mut dex_paths = Vec::new();
    collect_dex_files(fixture_dir, &mut dex_paths);
    assert!(
        !dex_paths.is_empty(),
        "no .dex files found under {fixture_dir:?} — fixture directory missing?",
    );

    let mut methods_checked: usize = 0;
    let mut methods_empty: usize = 0;

    for path in &dex_paths {
        let data = std::fs::read(path)
            .unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"));
        let dex = match DexFile::parse(&data, None) {
            Ok(d) => d,
            Err(e) => {
                // Adversarial fixtures intentionally fail to parse; skip them.
                let _ = e;
                continue;
            }
        };

        for cd in &dex.class_defs {
            if cd.class_data_off == 0 {
                continue;
            }
            let class_data = match parse_class_data(&data, cd.class_data_off) {
                Ok(d) => d,
                Err(_) => continue,
            };
            for em in class_data
                .direct_methods
                .iter()
                .chain(class_data.virtual_methods.iter())
            {
                if em.code_off == 0 {
                    continue;
                }
                let code_item = match parse_code_item(&data, em.code_off) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let cfg = match Cfg::build(&code_item) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                if cfg.blocks.is_empty() {
                    methods_empty += 1;
                    continue;
                }
                assert_transpose_invariant(&cfg);
                methods_checked += 1;
            }
        }
    }

    eprintln!(
        "transpose_real_corpus_fixtures: checked {methods_checked} non-empty CFGs, \
         {methods_empty} empty CFGs across {} DEX files",
        dex_paths.len(),
    );
    // At least one non-trivial CFG should have been checked.
    assert!(
        methods_checked > 0,
        "no non-empty CFGs found in fixture DEX files — \
         fixtures may be degenerate or all methods have no code",
    );
}

/// Recursively collect `.dex` paths under `dir`.
fn collect_dex_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_dex_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("dex") {
            out.push(path);
        }
    }
    out.sort();
}
