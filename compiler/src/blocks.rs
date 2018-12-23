use std::collections::BTreeMap;

use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::Direction;

use crate::translate::{BasicBlock, BlockId, BranchStub, VarId};

pub(crate) struct BlockGraph {
    inner: StableGraph<BasicBlock, ()>,
    addr_map: BTreeMap<BlockId, NodeIndex>,
}

impl BlockGraph {
    pub fn new() -> Self {
        BlockGraph {
            inner: StableGraph::new(),
            addr_map: BTreeMap::new(),
        }
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
                BranchStub::Goto(addr) => {
                    new_edges.push((*index, self.addr_map[&addr]));
                }
                BranchStub::IfICmp(_, _, _, if_addr, else_addr)
                | BranchStub::IfACmp(_, _, _, if_addr, else_addr) => {
                    new_edges.push((*index, self.addr_map[&if_addr]));
                    new_edges.push((*index, self.addr_map[&else_addr]));
                }
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

    pub fn phis(&self, block: &BasicBlock) -> BTreeMap<VarId, Vec<(VarId, BlockId)>> {
        let mut phis = BTreeMap::<VarId, Vec<(VarId, BlockId)>>::new();
        for incoming_block in self.incoming(block.address) {
            for (i, out_var) in incoming_block.outgoing.stack.iter().enumerate() {
                let var = &block.incoming.stack[i];
                phis.entry(var.clone())
                    .or_default()
                    .push((out_var.clone(), incoming_block.address));
            }
            for (i, out_var) in incoming_block.outgoing.locals.iter() {
                let var = &block.incoming.locals[i];
                phis.entry(var.clone())
                    .or_default()
                    .push((out_var.clone(), incoming_block.address));
            }
        }
        phis
    }
}
