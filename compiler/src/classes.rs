use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use classfile::constant_pool::Constant;
use classfile::{ClassFile, ConstantPool};
use failure::Fallible;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;

use loader::{ArrayClass, Class, ClassLoader};

#[derive(Debug)]
struct Inner {
    graph: StableGraph<Class, ()>,
    name_map: HashMap<String, NodeIndex>,
}

impl Inner {
    fn add_class(&mut self, name: &str, class: Class) -> NodeIndex {
        let idx = self.graph.add_node(class);
        self.name_map.insert(name.to_owned(), idx);
        idx
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ClassGraph {
    inner: Arc<RwLock<Inner>>,
}

impl ClassGraph {
    fn new() -> Self {
        let graph = StableGraph::new();
        let name_map = HashMap::new();
        let inner = Inner { graph, name_map };

        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    fn get_or_load_class<L>(&self, name: &str, loader: &L) -> Fallible<(NodeIndex, bool)>
    where
        L: ClassLoader,
    {
        let mut inner = self.inner.write().unwrap();
        if let Some(idx) = inner.name_map.get(name).cloned() {
            Ok((idx, false))
        } else {
            let class = loader.load(name)?;
            let idx = inner.add_class(name, class);
            Ok((idx, true))
        }
    }

    fn resolve_dependencies<L: ClassLoader>(
        &self,
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
                            let inner = self.inner.read().unwrap();
                            let mut class = &inner.graph[class_idx];
                            loop {
                                match class {
                                    Class::File(class_file) => {
                                        stack.push(class_file.constant_pool.clone());
                                        break;
                                    }
                                    Class::Array(ArrayClass::Primitive(_)) => {
                                        break;
                                    }
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
        let graph = Self::new();
        let pool = root.constant_pool.clone();
        let name = pool.get_utf8(root.get_this_class().name_index).unwrap();
        let _root_idx = graph
            .inner
            .write()
            .unwrap()
            .add_class(name, Class::File(Arc::new(root)));
        graph.resolve_dependencies(&pool, loader)?;
        Ok(graph)
    }

    pub fn get(&self, name: &str) -> Option<Class> {
        let inner = self.inner.read().unwrap();
        if let Some(idx) = inner.name_map.get(name) {
            Some(inner.graph[*idx].clone())
        } else {
            None
        }
    }
}
