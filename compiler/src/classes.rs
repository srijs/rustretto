use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use failure::Fallible;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;

use crate::loader::{ArrayClass, Class, ClassLoader};

#[derive(Debug)]
enum Relation {
    Extends,
    Implements,
}

struct Inner {
    graph: StableGraph<Class, Relation>,
    name_map: HashMap<String, NodeIndex>,
}

impl Inner {
    fn add_class(
        &mut self,
        name: &str,
        class: Class,
        loader: &dyn ClassLoader,
    ) -> Fallible<NodeIndex> {
        let index = self.graph.add_node(class.clone());
        self.name_map.insert(name.to_owned(), index);

        match class {
            Class::Array(ArrayClass::Primitive(_)) => {}
            Class::Array(ArrayClass::Complex(component_class)) => {
                self.graph.add_node(*component_class);
            }
            Class::File(class_file) => {
                if let Some(super_class_const) = class_file.get_super_class() {
                    let super_class_name = class_file
                        .constant_pool
                        .get_utf8(super_class_const.name_index)
                        .unwrap();
                    let super_class = loader.load(&super_class_name)?;
                    let super_index = self.add_class(&super_class_name, super_class, loader)?;
                    self.graph.add_edge(index, super_index, Relation::Extends);
                }
            }
        }

        Ok(index)
    }
}

#[derive(Clone)]
pub(crate) struct ClassGraph {
    inner: Arc<Mutex<Inner>>,
    loader: Arc<ClassLoader + Sync + Send>,
}

impl ClassGraph {
    pub fn new<L>(loader: L) -> Self
    where
        L: ClassLoader + Sync + Send + 'static,
    {
        let graph = StableGraph::new();
        let name_map = HashMap::new();
        let inner = Inner { graph, name_map };

        Self {
            inner: Arc::new(Mutex::new(inner)),
            loader: Arc::new(loader),
        }
    }

    pub fn get(&self, name: &str) -> Fallible<Class> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(idx) = inner.name_map.get(name).cloned() {
            Ok(inner.graph[idx].clone())
        } else {
            let class = self.loader.load(name)?;
            let idx = inner.add_class(name, class, &*self.loader)?;
            Ok(inner.graph[idx].clone())
        }
    }
}
