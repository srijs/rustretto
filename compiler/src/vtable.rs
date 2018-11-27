use std::sync::Arc;

use classfile::{ClassFile, MethodAccessFlags, MethodDescriptor};
use failure::Fallible;
use indexmap::IndexMap;

use classes::ClassGraph;
use loader::Class;

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

#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) struct MethodDispatchKey {
    pub method_name: String,
    pub method_descriptor: MethodDescriptor,
}

#[derive(Debug)]
pub(crate) struct MethodDispatchTarget {
    pub class_name: String,
    pub is_abstract: bool,
    pub is_override: bool,
}

#[derive(Debug)]
pub(crate) struct VTable {
    pub classfile: Arc<ClassFile>,
    pub method_dispatch: IndexMap<MethodDispatchKey, MethodDispatchTarget>,
}

impl VTable {
    pub fn new(name: &str, classes: &ClassGraph) -> Fallible<Self> {
        let classfile = match classes.get(name)? {
            Class::File(classfile) => classfile,
            Class::Array(_) => bail!("can't build vtable for array"),
        };

        let mut method_dispatch = IndexMap::new();
        if let Some(super_class) = classfile.get_super_class() {
            let super_class_name = classfile
                .constant_pool
                .get_utf8(super_class.name_index)
                .unwrap();
            let super_class_vtable = VTable::new(super_class_name, classes)?;
            method_dispatch = super_class_vtable.method_dispatch;
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
                .to_owned();
            let key = MethodDispatchKey {
                method_name,
                method_descriptor: method.descriptor.clone(),
            };
            let is_abstract = method.access_flags.contains(MethodAccessFlags::ABSTRACT);
            let is_override = method_dispatch.contains_key(&key);
            method_dispatch.insert(
                key,
                MethodDispatchTarget {
                    class_name: classfile.get_name().to_owned(),
                    is_abstract,
                    is_override,
                },
            );
        }

        Ok(VTable {
            classfile,
            method_dispatch,
        })
    }
}
