use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use classfile::FieldType;
use failure::{bail, Fallible};
use indexmap::{Equivalent, IndexMap};
use strbuf::StrBuf;

use crate::classes::ClassGraph;
use crate::loader::Class;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FieldAccessKey {
    pub field_name: StrBuf,
    pub field_type: FieldType,
}

impl Hash for FieldAccessKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.field_name.as_bytes());
        self.field_type.hash(state);
    }
}

struct LookupKey<'a> {
    field_name: &'a str,
    field_type: &'a FieldType,
}

impl<'a> Hash for LookupKey<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.field_name.as_bytes());
        self.field_type.hash(state);
    }
}

impl<'a> Equivalent<FieldAccessKey> for LookupKey<'a> {
    fn equivalent(&self, key: &FieldAccessKey) -> bool {
        self.field_name == &*key.field_name && self.field_type == &key.field_type
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FieldLayout {
    table: Arc<IndexMap<FieldAccessKey, ()>>,
}

impl FieldLayout {
    pub fn iter(&self) -> impl Iterator<Item = &FieldAccessKey> {
        self.table.keys()
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn get(&self, field_name: &str, field_type: &FieldType) -> Option<usize> {
        let key = LookupKey {
            field_name,
            field_type,
        };
        if let Some((idx, _, _)) = self.table.get_full(&key) {
            Some(idx)
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub(crate) struct FieldLayoutMap {
    classes: ClassGraph,
    inner: Arc<Mutex<HashMap<StrBuf, FieldLayout>>>,
}

impl FieldLayoutMap {
    pub fn new(classes: ClassGraph) -> Self {
        FieldLayoutMap {
            classes,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get(&self, name: &StrBuf) -> Fallible<FieldLayout> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.contains_key(name) {
            let mut table = IndexMap::new();
            self.build_table(name, &mut table)?;
            let layout = FieldLayout {
                table: Arc::new(table),
            };
            inner.insert(name.to_owned(), layout);
        }
        Ok(inner[name].clone())
    }

    fn build_table(&self, name: &StrBuf, table: &mut IndexMap<FieldAccessKey, ()>) -> Fallible<()> {
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

        for field in classfile.fields.iter() {
            // skip static methods
            if field.is_static() {
                continue;
            }

            let field_name = classfile
                .constant_pool
                .get_utf8(field.name_index)
                .unwrap()
                .clone();

            let key = FieldAccessKey {
                field_name,
                field_type: field.descriptor.clone(),
            };
            table.insert(key, ());
        }

        Ok(())
    }
}
