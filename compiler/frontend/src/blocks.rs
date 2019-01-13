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

#[derive(Debug)]
pub struct PhiOperand {
    pub opt: Option<Op>,
    pub src: PhiOperandSource,
}

pub struct PhiNode {
    pub target: VarId,
    pub operands: Vec<PhiOperand>,
}

impl PhiNode {
    fn new(target: VarId) -> Self {
        Self {
            target,
            operands: vec![],
        }
    }

    fn add(&mut self, opt: Option<&Op>, src: PhiOperandSource) {
        let operand;
        if let Some(op) = opt {
            if !self.target.0.can_unify_naive(&op.get_type()) {
                operand = PhiOperand { opt: None, src };
            } else {
                operand = PhiOperand {
                    opt: Some(op.clone()),
                    src,
                };
            }
        } else {
            operand = PhiOperand { opt: None, src };
        }
        log::trace!(
            "adding binding {:?} for variable {:?}",
            operand,
            self.target
        );

        self.operands.push(operand);
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

    fn incoming_frames(
        &self,
        block: &BasicBlock,
    ) -> impl Iterator<Item = (PhiOperandSource, &StackAndLocals)> {
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

        incoming_frames.chain(entry_frame)
    }

    pub fn phis(&self, block: &BasicBlock) -> impl Iterator<Item = PhiNode> {
        log::trace!(
            "collecting phi nodes for block at address {}",
            block.address
        );
        let mut nodes = Vec::new();

        for (i, in_op) in block.incoming.stack.iter().enumerate() {
            if let Op::Var(in_var) = in_op {
                let mut node = PhiNode::new(in_var.clone());
                log::trace!("processing incoming stack variable ({}={:?})", i, in_var);
                for (src, frame) in self.incoming_frames(block) {
                    log::trace!("matching up incoming frame (src={:?})", src);
                    node.add(frame.stack.get(i), src.clone());
                }
                nodes.push(node);
            }
        }

        for (i, in_op) in block.incoming.locals.iter() {
            if let Op::Var(in_var) = in_op {
                let mut node = PhiNode::new(in_var.clone());
                log::trace!("processing incoming local variable ({}={:?})", i, in_var);
                for (src, frame) in self.incoming_frames(block) {
                    log::trace!("matching up incoming frame (src={:?})", src);
                    node.add(frame.locals.get(i), src.clone());
                }
                nodes.push(node);
            }
        }

        nodes.into_iter()
    }
}
