use std::collections::BTreeMap;

use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::Direction;

use crate::frame::StackAndLocals;
use crate::translate::{BasicBlock, BlockId, BranchStub, Op, VarId};

pub struct BlockGraph {
    inner: StableGraph<BasicBlock, ()>,
    addr_map: BTreeMap<BlockId, NodeIndex>,
    entry_state: StackAndLocals,
}

#[derive(Clone, Debug)]
pub enum PhiOperandSource {
    Entry,
    Block(BlockId),
}

pub struct PhiOperand {
    pub op: Op,
    pub src: PhiOperandSource,
}

pub struct PhiMap {
    inner: BTreeMap<VarId, Option<Vec<PhiOperand>>>,
}

impl PhiMap {
    fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    fn add(&mut self, target: &Op, operand: PhiOperand) {
        if let Op::Var(target_var) = target {
            if !target_var.0.can_unify_naive(&operand.op.get_type()) {
                // mark as unusable
                self.inner.insert(target_var.clone(), None);
            } else {
                log::trace!(
                    "adding binding {:?} from {:?} for variable {:?}",
                    operand.op,
                    operand.src,
                    target_var
                );

                let entry = self
                    .inner
                    .entry(target_var.clone())
                    .or_insert_with(|| Some(Vec::new()));

                if let Some(ref mut operands) = entry {
                    operands.push(operand);
                }
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&VarId, &[PhiOperand])> {
        self.inner
            .iter()
            .filter_map(|(var, opt)| opt.as_ref().map(|operands| (var, operands.as_slice())))
    }
}

impl BlockGraph {
    pub fn new(entry_state: StackAndLocals) -> Self {
        BlockGraph {
            entry_state,
            inner: StableGraph::default(),
            addr_map: BTreeMap::default(),
        }
    }

    pub fn entry(&self) -> &StackAndLocals {
        &self.entry_state
    }

    pub fn contains(&self, addr: BlockId) -> bool {
        self.addr_map.contains_key(&addr)
    }

    pub fn lookup(&self, addr: BlockId) -> &BasicBlock {
        let index = self.addr_map[&addr];
        &self.inner[index]
    }

    pub fn incoming(&self, addr: BlockId) -> impl Iterator<Item = &BasicBlock> {
        let index = self.addr_map[&addr];
        self.inner
            .neighbors_directed(index, Direction::Incoming)
            .map(move |neighbor_index| &self.inner[neighbor_index])
    }

    pub fn blocks(&self) -> impl Iterator<Item = &BasicBlock> {
        self.inner
            .node_indices()
            .map(move |index| &self.inner[index])
    }

    pub fn insert(&mut self, block: BasicBlock) {
        let address = block.address;
        let index = self.inner.add_node(block);
        self.addr_map.insert(address, index);
    }

    pub fn calculate_edges(&mut self) {
        let mut new_edges = vec![];
        for (_, index) in self.addr_map.iter() {
            match self.inner[*index].branch_stub {
                BranchStub::Switch(ref switch) => {
                    new_edges.push((*index, self.addr_map[&switch.default]));
                    for (_, addr) in switch.cases.iter() {
                        new_edges.push((*index, self.addr_map[&addr]));
                    }
                }
                _ => {}
            }
            for (a, b) in new_edges.drain(..) {
                self.inner.update_edge(a, b, ());
            }
        }
    }

    pub fn phis(&self, block: &BasicBlock) -> PhiMap {
        log::trace!(
            "collecting phi nodes for block at address {}",
            block.address
        );
        let mut phis = PhiMap::new();

        let entry_frame = if block.address == BlockId::start() {
            Some((PhiOperandSource::Entry, &self.entry_state))
        } else {
            None
        };

        let incoming_frames = self.incoming(block.address).map(|incoming_block| {
            (
                PhiOperandSource::Block(incoming_block.address),
                &incoming_block.outgoing,
            )
        });

        for (src, frame) in incoming_frames.chain(entry_frame) {
            log::trace!("matching up incoming frame (src={:?})", src);
            for (i, out_var) in frame.stack.iter().enumerate() {
                log::trace!("looking up incoming stack variable ({}={:?})", i, out_var);
                if let Some(op) = block.incoming.stack.get(i) {
                    phis.add(
                        op,
                        PhiOperand {
                            op: out_var.clone(),
                            src: src.clone(),
                        },
                    );
                }
            }
            for (i, out_var) in frame.locals.iter() {
                log::trace!("looking up incoming local variable ({}={:?})", i, out_var);
                if let Some(op) = block.incoming.locals.get(i) {
                    phis.add(
                        op,
                        PhiOperand {
                            op: out_var.clone(),
                            src: src.clone(),
                        },
                    );
                }
            }
        }

        phis
    }
}
