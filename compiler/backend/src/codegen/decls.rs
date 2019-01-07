use std::fmt::{self, Write};
use std::sync::Arc;

use failure::Fallible;
use fnv::FnvBuildHasher;
use indexmap::IndexMap;
use strbuf::StrBuf;

use frontend::classes::ClassGraph;
use frontend::loader::{ArrayClass, Class};

use crate::codegen::common::*;
use crate::layout::{FieldLayoutMap, VTableMap};
use crate::mangle;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum DeclKey {
    VTableType { class_name: StrBuf },
    ObjectType { class_name: StrBuf },
}

#[derive(Clone, Debug)]
struct DeclEntry {
    global: bool,
    identifier: Arc<String>,
    declaration: String,
}

pub struct DeclIdentifier {
    global: bool,
    identifier: Arc<String>,
}

impl fmt::Display for DeclIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.global {
            write!(f, "@{}", self.identifier)
        } else {
            write!(f, "%{}", self.identifier)
        }
    }
}

pub struct DeclDatabase {
    classes: ClassGraph,
    vtables: VTableMap,
    field_layouts: FieldLayoutMap,
    decls: IndexMap<DeclKey, DeclEntry, FnvBuildHasher>,
}

impl DeclDatabase {
    pub fn new(classes: &ClassGraph, vtables: &VTableMap, field_layouts: &FieldLayoutMap) -> Self {
        Self {
            classes: classes.clone(),
            vtables: vtables.clone(),
            field_layouts: field_layouts.clone(),
            decls: IndexMap::default(),
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = &str> {
        self.decls.values().map(|entry| &*entry.declaration)
    }

    pub fn add_object_type(&mut self, class_name: &StrBuf) -> Fallible<DeclIdentifier> {
        self.add(DeclKey::ObjectType {
            class_name: class_name.clone(),
        })
    }

    pub fn add_vtable_type(&mut self, class_name: &StrBuf) -> Fallible<DeclIdentifier> {
        self.add(DeclKey::VTableType {
            class_name: class_name.clone(),
        })
    }

    fn add(&mut self, key: DeclKey) -> Fallible<DeclIdentifier> {
        if let Some(entry) = self.decls.get(&key) {
            return Ok(DeclIdentifier {
                global: entry.global,
                identifier: entry.identifier.clone(),
            });
        }
        let mut gen = DeclGen {
            out: String::new(),
            classes: &self.classes,
            vtables: &self.vtables,
            field_layouts: &self.field_layouts,
        };
        let identifier = match key {
            DeclKey::VTableType { ref class_name } => gen.gen_vtable_type(class_name)?,
            DeclKey::ObjectType { ref class_name } => gen.gen_object_type(class_name)?,
        };
        self.decls.insert(
            key,
            DeclEntry {
                global: identifier.global,
                identifier: identifier.identifier.clone(),
                declaration: gen.out,
            },
        );
        Ok(identifier)
    }
}

struct DeclGen<'a> {
    out: String,
    classes: &'a ClassGraph,
    vtables: &'a VTableMap,
    field_layouts: &'a FieldLayoutMap,
}

impl<'a> DeclGen<'a> {
    fn gen_vtable_type(&mut self, class_name: &StrBuf) -> Fallible<DeclIdentifier> {
        let vtable = self.vtables.get(class_name)?;
        let vtable_name = mangle::mangle_vtable_name(class_name);
        writeln!(self.out, "%{} = type {{", vtable_name)?;
        write!(self.out, "  i32")?;
        if !vtable.is_empty() {
            write!(self.out, ",")?;
        }
        writeln!(self.out, " ; <number of table entries>")?;
        for (idx, (key, _)) in vtable.iter().enumerate() {
            let ftyp = tlt_function_type(&key.method_descriptor);
            write!(self.out, "  {} *", ftyp)?;
            if idx < vtable.len() - 1 {
                write!(self.out, ",")?;
            } else {
                write!(self.out, "")?;
            }
            writeln!(self.out, " ; {}", key.method_name)?;
        }
        writeln!(self.out, "}}")?;

        Ok(DeclIdentifier {
            global: false,
            identifier: Arc::new(vtable_name),
        })
    }

    fn gen_object_type(&mut self, class_name: &StrBuf) -> Fallible<DeclIdentifier> {
        match self.classes.get(class_name)? {
            Class::File(_) => self.gen_object_struct_type(class_name),
            Class::Array(array_class) => self.gen_object_array_type(class_name, &array_class),
        }
    }

    fn gen_object_array_type(
        &mut self,
        class_name: &StrBuf,
        array_class: &ArrayClass,
    ) -> Fallible<DeclIdentifier> {
        let object_type_name = mangle::mangle_class_name(class_name);
        writeln!(self.out, "%{} = type {{", object_type_name)?;
        writeln!(self.out, "  i32, ; length")?;
        writeln!(
            self.out,
            "  [0 x {}] ; members",
            tlt_array_class_component_type(array_class)
        )?;
        writeln!(self.out, "}}")?;
        Ok(DeclIdentifier {
            global: false,
            identifier: Arc::new(object_type_name),
        })
    }

    fn gen_object_struct_type(&mut self, class_name: &StrBuf) -> Fallible<DeclIdentifier> {
        let field_layout = self.field_layouts.get(class_name)?;
        let object_type_name = mangle::mangle_class_name(class_name);
        writeln!(self.out, "%{} = type {{", object_type_name)?;
        for (idx, key) in field_layout.iter().enumerate() {
            let ftyp = tlt_field_type(&key.field_type);
            write!(self.out, "  {}", ftyp)?;
            if idx < field_layout.len() - 1 {
                write!(self.out, ",")?;
            } else {
                write!(self.out, "")?;
            }
            writeln!(self.out, " ; {}", key.field_name)?;
        }
        writeln!(self.out, "}}")?;
        Ok(DeclIdentifier {
            global: false,
            identifier: Arc::new(object_type_name),
        })
    }
}
