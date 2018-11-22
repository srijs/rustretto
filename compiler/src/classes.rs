use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use classfile::ClassFile;
use failure::Fallible;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;

use loader::{BootstrapClassLoader, Class, ClassLoader};

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
    inner: Arc<Mutex<Inner>>,
    loader: BootstrapClassLoader,
}

impl ClassGraph {
    fn new(loader: BootstrapClassLoader) -> Self {
        let graph = StableGraph::new();
        let name_map = HashMap::new();
        let inner = Inner { graph, name_map };

        Self {
            inner: Arc::new(Mutex::new(inner)),
            loader,
        }
    }

    pub fn get(&self, name: &str) -> Fallible<Class> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(idx) = inner.name_map.get(name).cloned() {
            Ok(inner.graph[idx].clone())
        } else {
            let class = self.loader.load(name)?;
            let idx = inner.add_class(name, class);
            Ok(inner.graph[idx].clone())
        }
    }

    pub fn build(root: ClassFile, loader: BootstrapClassLoader) -> Fallible<Self> {
        let graph = Self::new(loader);
        let pool = root.constant_pool.clone();
        let name = pool.get_utf8(root.get_this_class().name_index).unwrap();
        let _root_idx = graph
            .inner
            .lock()
            .unwrap()
            .add_class(name, Class::File(Arc::new(root)));
        Ok(graph)
    }
}
