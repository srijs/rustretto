use std::collections::BTreeMap;

use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::Direction;

use crate::translate::{BasicBlock, BlockId, BranchStub, Op, VarId};

#[derive(Default)]
pub struct BlockGraph {
    inner: StableGraph<BasicBlock, ()>,
    addr_map: BTreeMap<BlockId, NodeIndex>,
}

pub struct PhiMap {
    inner: BTreeMap<VarId, Option<Vec<(Op, BlockId)>>>,
}

impl PhiMap {
    fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    fn add(&mut self, left_op: &Op, right_op: &Op, addr: BlockId) {
        if let Op::Var(left_var) = left_op {
            if !left_var.0.can_unify_naive(&right_op.get_type()) {
                // mark as unusable
                self.inner.insert(left_var.clone(), None);
            } else {
                log::trace!(
                    "adding binding {:?} from block {} for variable {:?}",
                    right_op,
                    addr,
                    left_var
                );

                let entry = self
                    .inner
                    .entry(left_var.clone())
                    .or_insert_with(|| Some(Vec::new()));

                if let Some(ref mut bindings) = entry {
                    bindings.push((right_op.clone(), addr));
                }
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&VarId, &[(Op, BlockId)])> {
        self.inner
            .iter()
            .filter_map(|(var, opt)| opt.as_ref().map(|bindings| (var, bindings.as_slice())))
    }
}

impl BlockGraph {
    pub fn new() -> Self {
        BlockGraph::default()
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
        for incoming_block in self.incoming(block.address) {
            log::trace!(
                "matching up incoming block at address {}",
                incoming_block.address
            );
            for (i, out_var) in incoming_block.outgoing.stack.iter().enumerate() {
                log::trace!("looking up incoming stack variable ({}={:?})", i, out_var);
                if let Some(op) = block.incoming.stack.get(i) {
                    phis.add(op, out_var, incoming_block.address);
                }
            }
            for (i, out_var) in incoming_block.outgoing.locals.iter() {
                log::trace!("looking up incoming local variable ({}={:?})", i, out_var);
                if let Some(op) = block.incoming.locals.get(i) {
                    phis.add(op, out_var, incoming_block.address);
                }
            }
        }
        phis
    }
}
