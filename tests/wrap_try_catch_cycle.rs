//! Regression test: `wrap_try_catch` must terminate on handler bodies whose
//! goto-follow chain forms a cycle. Without a visited-set, the walker loops
//! forever and grows `handler_insns` unboundedly — a DoS primitive against any
//! tool that runs the decompile pipeline over attacker-controlled DEX.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use droidsaw_dex::cfg::{BasicBlock, BlockIdx, Cfg, Edge, EdgeKind, ExceptionRegion};
use droidsaw_dex::decode::{Instruction, RegList};
use droidsaw_dex::opcodes::Opcode;
use droidsaw_dex::ssa::{SsaBlock, SsaBody, SsaInsn};
use droidsaw_dex::structure::{wrap_try_catch, Stmt};

fn insn(addr: u32, op: Opcode) -> Instruction {
    Instruction {
        addr,
        op,
        size: 1,
        dst: None,
        src: RegList::empty(),
        literal: 0,
        target: None,
        pool_idx: None,
    }
}

fn ssa_insn(addr: u32, op: Opcode) -> SsaInsn {
    SsaInsn {
        insn: insn(addr, op),
        dst: None,
        uses: vec![],
    }
}

#[test]
fn wrap_try_catch_terminates_on_cyclic_handler() {
    // CFG:
    //   block 0: try body (NOP, Return)
    //   block 1: handler entry (MoveException, Goto → block 2)
    //   block 2: (Nop, Goto → block 1)  ← cycle back into handler
    //
    // Without a visited-set in wrap_try_catch's goto-chain walker, this
    // follows 1 → 2 → 1 → 2 → … forever and allocates unboundedly.
    let blocks = vec![
        BasicBlock {
            id: BlockIdx(0),
            start_addr: 0,
            instructions: vec![insn(0, Opcode::Nop), insn(1, Opcode::ReturnVoid)],
            successors: vec![],
            predecessors: BTreeSet::new(),
        },
        BasicBlock {
            id: BlockIdx(1),
            start_addr: 10,
            instructions: vec![insn(10, Opcode::MoveException), insn(11, Opcode::Goto)],
            successors: vec![Edge {
                target: BlockIdx(2),
                kind: EdgeKind::Branch,
            }],
            predecessors: BTreeSet::new(),
        },
        BasicBlock {
            id: BlockIdx(2),
            start_addr: 20,
            instructions: vec![insn(20, Opcode::Nop), insn(21, Opcode::Goto)],
            successors: vec![Edge {
                target: BlockIdx(1),
                kind: EdgeKind::Branch,
            }],
            predecessors: BTreeSet::new(),
        },
    ];
    let cfg = Cfg {
        blocks,
        entry: BlockIdx(0),
        exception_regions: vec![ExceptionRegion {
            start_addr: 0,
            end_addr: 2,
            handler_blocks: vec![(EdgeKind::ExceptionCatchAll, BlockIdx(1))],
        }],
        addr_to_block: BTreeMap::new(),
    };
    let mut ssa_blocks = BTreeMap::new();
    ssa_blocks.insert(
        BlockIdx(0),
        SsaBlock {
            id: BlockIdx(0),
            phis: vec![],
            insns: vec![ssa_insn(0, Opcode::Nop), ssa_insn(1, Opcode::ReturnVoid)],
        },
    );
    ssa_blocks.insert(
        BlockIdx(1),
        SsaBlock {
            id: BlockIdx(1),
            phis: vec![],
            insns: vec![ssa_insn(10, Opcode::MoveException), ssa_insn(11, Opcode::Goto)],
        },
    );
    ssa_blocks.insert(
        BlockIdx(2),
        SsaBlock {
            id: BlockIdx(2),
            phis: vec![],
            insns: vec![ssa_insn(20, Opcode::Nop), ssa_insn(21, Opcode::Goto)],
        },
    );
    let ssa = SsaBody {
        blocks: ssa_blocks,
        entry: BlockIdx(0),
        var_counter: 0,
        param_vars: vec![],
    };

    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let out = wrap_try_catch(Stmt::Seq(vec![]), &cfg, &ssa);
        let _ = tx.send(out);
    });

    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(_) => {
            // Join to make sure the worker thread has exited cleanly.
            let _ = handle.join();
        }
        Err(_) => {
            panic!("wrap_try_catch hung on cyclic handler — visited-set protection missing");
        }
    }
}
