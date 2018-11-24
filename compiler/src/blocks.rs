use std::collections::BTreeMap;

use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::Direction;

use translate::{BasicBlock, BranchStub};

pub(crate) struct BlockGraph {
    inner: StableGraph<BasicBlock, ()>,
    addr_map: BTreeMap<u32, NodeIndex>,
}

impl BlockGraph {
    pub fn new() -> Self {
        BlockGraph {
            inner: StableGraph::new(),
            addr_map: BTreeMap::new(),
        }
    }

    pub fn contains(&self, addr: u32) -> bool {
        self.addr_map.contains_key(&addr)
    }

    pub fn lookup(&self, addr: u32) -> &BasicBlock {
        let index = self.addr_map[&addr];
        &self.inner[index]
    }

    pub fn incoming(&self, addr: u32) -> impl Iterator<Item = &BasicBlock> {
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
        for (_, index) in self.addr_map.iter() {
            match self.inner[*index].branch_stub {
                BranchStub::Goto(addr) => {
                    self.inner.update_edge(*index, self.addr_map[&addr], ());
                }
                BranchStub::IfEq(_, if_addr, else_addr) => {
                    self.inner.update_edge(*index, self.addr_map[&if_addr], ());
                    self.inner
                        .update_edge(*index, self.addr_map[&else_addr], ());
                }
                BranchStub::Invoke(_, _, addr) => {
                    self.inner.update_edge(*index, self.addr_map[&addr], ());
                }
                _ => {}
            }
        }
    }
}