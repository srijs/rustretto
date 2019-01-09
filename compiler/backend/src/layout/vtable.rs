use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use classfile::MethodDescriptor;
use failure::{bail, Fallible};
use fnv::{FnvBuildHasher, FnvHashMap};
use indexmap::{map::Entry as IndexMapEntry, Equivalent, IndexMap};
use strbuf::StrBuf;

use frontend::classes::ClassGraph;
use frontend::loader::Class;

/*

type info:
- serve as a template for generating vtable types
- serve as data source for narrowing to sub-vtables
  - needed when coercing to a supertype or interface type
- serve as an index when extracting function pointers for calling

value info:
- serve as a data source for generating vtable constants
  - map method name + descriptor to the implementing class

*/

#[derive(Debug, Eq)]
pub struct MethodDispatchKey {
    pub method_name: StrBuf,
    pub method_descriptor: MethodDescriptor,
}

impl Hash for MethodDispatchKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.method_name.as_bytes());
        self.method_descriptor.hash(state);
    }
}

impl PartialEq for MethodDispatchKey {
    fn eq(&self, other: &Self) -> bool {
        self.method_name == other.method_name && self.method_descriptor == other.method_descriptor
    }
}

struct LookupKey<'a> {
    method_name: &'a str,
    method_descriptor: &'a MethodDescriptor,
}

impl<'a> Hash for LookupKey<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.method_name.as_bytes());
        self.method_descriptor.hash(state);
    }
}

impl<'a> Equivalent<MethodDispatchKey> for LookupKey<'a> {
    fn equivalent(&self, key: &MethodDispatchKey) -> bool {
        self.method_name == &*key.method_name && self.method_descriptor == &key.method_descriptor
    }
}

#[derive(Clone, Debug)]
pub struct MethodDispatchTarget {
    pub class_name: StrBuf,
    pub method_index_upper: usize,
    pub method_index_lower: usize,
}

#[derive(Debug, Default)]
struct VTableInner {
    // map from method signatures to resolved targets
    target_map: IndexMap<MethodDispatchKey, MethodDispatchTarget, FnvBuildHasher>,
    // ordered list of indices into the target_map
    methods: Vec<usize>,
    // map from interface names to indices into the methods vector
    interfaces: FnvHashMap<StrBuf, usize>,
}

#[derive(Clone, Debug)]
pub struct VTable {
    inner: Arc<VTableInner>,
}

impl VTable {
    pub fn iter_methods(
        &self,
    ) -> impl Iterator<Item = (&MethodDispatchKey, &MethodDispatchTarget)> {
        self.inner
            .methods
            .iter()
            .map(move |idx| self.inner.target_map.get_index(*idx).unwrap())
    }

    pub fn iter_interfaces(&self) -> impl Iterator<Item = (&StrBuf, &usize)> {
        self.inner.interfaces.iter()
    }

    pub fn method_count(&self) -> usize {
        self.inner.methods.len()
    }

    pub fn interface_count(&self) -> usize {
        self.inner.interfaces.len()
    }

    pub fn get(
        &self,
        method_name: &str,
        method_descriptor: &MethodDescriptor,
    ) -> Option<&MethodDispatchTarget> {
        let key = LookupKey {
            method_name,
            method_descriptor,
        };
        self.inner.target_map.get(&key)
    }
}

#[derive(Clone)]
pub struct VTableMap {
    classes: ClassGraph,
    inner: Arc<Mutex<FnvHashMap<StrBuf, VTable>>>,
}

impl VTableMap {
    pub fn new(classes: ClassGraph) -> Self {
        VTableMap {
            classes,
            inner: Arc::new(Mutex::new(FnvHashMap::default())),
        }
    }

    pub fn get(&self, name: &StrBuf) -> Fallible<VTable> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.contains_key(name) {
            let mut table_inner = VTableInner::default();
            self.build_table(name, &mut table_inner, 0)?;
            let vtable = VTable {
                inner: Arc::new(table_inner),
            };
            inner.insert(name.to_owned(), vtable);
        }
        Ok(inner[name].clone())
    }

    fn build_table(
        &self,
        name: &StrBuf,
        table_inner: &mut VTableInner,
        method_offset: usize,
    ) -> Fallible<()> {
        let classfile = match self.classes.get(name)? {
            Class::File(classfile) => classfile,
            Class::Array(_) => bail!("can't build vtable for array"),
        };

        if !classfile.is_interface() {
            if let Some(super_class) = classfile.get_super_class() {
                let super_class_name = classfile
                    .constant_pool
                    .get_utf8(super_class.name_index)
                    .unwrap();
                self.build_table(super_class_name, table_inner, method_offset)?;
            }
        }

        for cidx in classfile.interfaces.iter() {
            let interface_constant = classfile.constant_pool.get_class(*cidx).unwrap();
            let interface_name = classfile
                .constant_pool
                .get_utf8(interface_constant.name_index)
                .unwrap();
            if let Some(method_index) = table_inner.interfaces.get(&*interface_name) {
                // skip interfaces that are already implemented by superclasses
                if *method_index >= method_offset {
                    continue;
                }
            }
            let interface_method_offset = table_inner.methods.len();
            self.build_table(interface_name, table_inner, interface_method_offset)?;
            table_inner
                .interfaces
                .insert(interface_name.clone(), interface_method_offset);
        }

        for method in classfile.methods.iter() {
            // skip static methods
            if method.is_static() {
                continue;
            }

            let method_name = classfile
                .constant_pool
                .get_utf8(method.name_index)
                .unwrap()
                .clone();

            // skip instance initialization methods
            if &*method_name == "<init>" {
                continue;
            }

            let key = MethodDispatchKey {
                method_name,
                method_descriptor: method.descriptor.clone(),
            };

            let class_name = classfile.get_name().to_owned();
            let method_index = table_inner.methods.len();

            match table_inner.target_map.entry(key) {
                IndexMapEntry::Vacant(entry) => {
                    table_inner.methods.push(entry.index());
                    entry.insert(MethodDispatchTarget {
                        class_name,
                        method_index_lower: method_index,
                        method_index_upper: method_index,
                    });
                }
                IndexMapEntry::Occupied(mut entry) => {
                    if entry.get().method_index_upper < method_offset {
                        entry.get_mut().method_index_upper = method_index;
                        table_inner.methods.push(entry.index());
                    }
                    if !classfile.is_interface() {
                        entry.get_mut().class_name = class_name;
                    }
                }
            }
        }

        Ok(())
    }
}
