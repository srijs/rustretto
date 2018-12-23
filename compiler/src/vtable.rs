use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use classfile::{MethodAccessFlags, MethodDescriptor};
use failure::{bail, Fallible};
use indexmap::{Equivalent, IndexMap};
use strbuf::StrBuf;

use crate::classes::ClassGraph;
use crate::loader::Class;

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

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MethodDispatchKey {
    pub method_name: StrBuf,
    pub method_descriptor: MethodDescriptor,
}

impl Hash for MethodDispatchKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.method_name.as_bytes());
        self.method_descriptor.hash(state);
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

#[derive(Debug)]
pub(crate) struct MethodDispatchTarget {
    pub class_name: StrBuf,
    pub is_abstract: bool,
    pub is_override: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct VTable {
    table: Arc<IndexMap<MethodDispatchKey, MethodDispatchTarget>>,
}

impl VTable {
    pub fn iter(&self) -> impl Iterator<Item = (&MethodDispatchKey, &MethodDispatchTarget)> {
        self.table.iter()
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn get(
        &self,
        method_name: &str,
        method_descriptor: &MethodDescriptor,
    ) -> Option<(usize, &MethodDispatchTarget)> {
        let key = LookupKey {
            method_name,
            method_descriptor,
        };
        if let Some((idx, _, target)) = self.table.get_full(&key) {
            Some((idx, target))
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub(crate) struct VTableMap {
    classes: ClassGraph,
    inner: Arc<Mutex<HashMap<StrBuf, VTable>>>,
}

impl VTableMap {
    pub fn new(classes: ClassGraph) -> Self {
        VTableMap {
            classes,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get(&self, name: &StrBuf) -> Fallible<VTable> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.contains_key(name) {
            let mut table = IndexMap::new();
            self.build_table(name, &mut table)?;
            let vtable = VTable {
                table: Arc::new(table),
            };
            inner.insert(name.to_owned(), vtable);
        }
        Ok(inner[name].clone())
    }

    fn build_table(
        &self,
        name: &str,
        table: &mut IndexMap<MethodDispatchKey, MethodDispatchTarget>,
    ) -> Fallible<()> {
        let classfile = match self.classes.get(name)? {
            Class::File(classfile) => classfile,
            Class::Array(_) => bail!("can't build vtable for array"),
        };

        if let Some(super_class) = classfile.get_super_class() {
            let super_class_name = classfile
                .constant_pool
                .get_utf8(super_class.name_index)
                .unwrap();
            self.build_table(super_class_name, table)?;
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
            let is_abstract = method.access_flags.contains(MethodAccessFlags::ABSTRACT);
            let is_override = table.contains_key(&key);
            table.insert(
                key,
                MethodDispatchTarget {
                    class_name: classfile.get_name().to_owned(),
                    is_abstract,
                    is_override,
                },
            );
        }

        Ok(())
    }
}
