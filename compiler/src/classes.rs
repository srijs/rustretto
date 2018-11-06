use std::collections::HashMap;

use classfile::constant_pool::Constant;
use classfile::{ClassFile, ConstantPool};
use failure::Fallible;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;

use loader::{Class, ArrayClass, ClassLoader};

pub(crate) struct ClassGraph {
    inner: StableGraph<Class, ()>,
    name_map: HashMap<String, NodeIndex>,
}

impl ClassGraph {
    fn new() -> Self {
        let inner = StableGraph::new();
        let name_map = HashMap::new();

        Self { inner, name_map }
    }

    fn add_class(&mut self, name: &str, class: Class) -> NodeIndex {
        let idx = self.inner.add_node(class);
        self.name_map.insert(name.to_owned(), idx);
        idx
    }

    fn get_or_load_class<L>(&mut self, name: &str, loader: &L) -> Fallible<(NodeIndex, bool)>
    where
        L: ClassLoader,
    {
        if let Some(idx) = self.name_map.get(name).cloned() {
            Ok((idx, false))
        } else {
            let class = loader.load(name)?;
            let idx = self.add_class(name, class);
            Ok((idx, true))
        }
    }

    fn resolve_dependencies<L: ClassLoader>(
        &mut self,
        pool: &ConstantPool,
        loader: &L,
    ) -> Fallible<()> {
        let mut stack = vec![pool.clone()];

        while let Some(constant_pool) = stack.pop() {
            for idx in constant_pool.indices() {
                match constant_pool.get_info(idx).unwrap() {
                    Constant::Class(class_constant) => {
                        let class_name = constant_pool.get_utf8(class_constant.name_index).unwrap();
                        let (class_idx, loaded) = self.get_or_load_class(class_name, loader)?;
                        if loaded {
                            let mut class = &self.inner[class_idx];
                            loop {
                                match class {
                                    Class::File(class_file) => {
                                        stack.push(class_file.constant_pool.clone());
                                        break;
                                    },
                                    Class::Array(ArrayClass::Primitive(_)) => {
                                        break;
                                    },
                                    Class::Array(ArrayClass::Complex(inner_class)) => {
                                        class = inner_class;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    pub fn build<L: ClassLoader>(root: ClassFile, loader: &L) -> Fallible<Self> {
        let mut graph = Self::new();
        let pool = root.constant_pool.clone();
        let name = pool.get_utf8(root.get_this_class().name_index).unwrap();
        let root_idx = graph.add_class(name, Class::File(root));
        graph.resolve_dependencies(&pool, loader)?;
        Ok(graph)
    }

    pub fn get(&self, name: &str) -> Option<&Class> {
        if let Some(idx) = self.name_map.get(name) {
            Some(&self.inner[*idx])
        } else {
            None
        }
    }
}
