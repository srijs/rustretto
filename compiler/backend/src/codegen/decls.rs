use std::fmt::{self, Write};
use std::sync::Arc;

use classfile::descriptors::{FieldType, MethodDescriptor, ParameterDescriptor};
use failure::{bail, Fallible};
use fnv::FnvBuildHasher;
use indexmap::IndexMap;
use strbuf::StrBuf;

use frontend::classes::ClassGraph;
use frontend::loader::{ArrayClass, Class};
use frontend::types::Type;

use crate::codegen::common::*;
use crate::layout::{FieldLayoutMap, VTableMap};
use crate::mangle;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum DeclKey {
    ObjectType {
        class_name: StrBuf,
    },
    VTableType {
        class_name: StrBuf,
    },
    VTableConst {
        class_name: StrBuf,
        vtable_type: DeclIdentifier,
    },
    Method {
        class_name: StrBuf,
        method_name: StrBuf,
        method_descriptor: MethodDescriptor,
        is_static: bool,
    },
    StaticField {
        class_name: StrBuf,
        field_name: StrBuf,
        field_type: FieldType,
    },
}

#[derive(Clone, Debug)]
struct DeclEntry {
    global: bool,
    identifier: Arc<String>,
    declaration: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

    pub fn add_vtable_const(&mut self, class_name: &StrBuf) -> Fallible<DeclIdentifier> {
        let vtable_type = self.add_vtable_type(class_name)?;
        self.add(DeclKey::VTableConst {
            class_name: class_name.clone(),
            vtable_type,
        })
    }

    pub fn add_instance_method(
        &mut self,
        class_name: &StrBuf,
        method_name: &StrBuf,
        method_descriptor: &MethodDescriptor,
    ) -> Fallible<DeclIdentifier> {
        self.add(DeclKey::Method {
            class_name: class_name.clone(),
            method_name: method_name.clone(),
            method_descriptor: method_descriptor.clone(),
            is_static: false,
        })
    }

    pub fn add_static_method(
        &mut self,
        class_name: &StrBuf,
        method_name: &StrBuf,
        method_descriptor: &MethodDescriptor,
    ) -> Fallible<DeclIdentifier> {
        self.add(DeclKey::Method {
            class_name: class_name.clone(),
            method_name: method_name.clone(),
            method_descriptor: method_descriptor.clone(),
            is_static: true,
        })
    }

    pub fn add_static_field(
        &mut self,
        class_name: &StrBuf,
        field_name: &StrBuf,
        field_type: &FieldType,
    ) -> Fallible<DeclIdentifier> {
        self.add(DeclKey::StaticField {
            class_name: class_name.clone(),
            field_name: field_name.clone(),
            field_type: field_type.clone(),
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
            DeclKey::ObjectType { ref class_name } => gen.gen_object_type(class_name)?,
            DeclKey::VTableType { ref class_name } => gen.gen_vtable_type(class_name)?,
            DeclKey::VTableConst {
                ref class_name,
                ref vtable_type,
            } => gen.gen_vtable_const(class_name, vtable_type)?,
            DeclKey::Method {
                ref class_name,
                ref method_name,
                ref method_descriptor,
                is_static,
            } => gen.gen_method(class_name, method_name, method_descriptor, is_static)?,
            DeclKey::StaticField {
                ref class_name,
                ref field_name,
                ref field_type,
            } => gen.gen_field(class_name, field_name, field_type)?,
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
    fn gen_field(
        &mut self,
        class_name: &StrBuf,
        field_name: &StrBuf,
        field_type: &FieldType,
    ) -> Fallible<DeclIdentifier> {
        let mangled_name = mangle::mangle_field_name(class_name, field_name);
        writeln!(
            self.out,
            "@{field_name} = external global {field_type}",
            field_name = mangled_name,
            field_type = tlt_field_type(field_type)
        )?;
        Ok(DeclIdentifier {
            global: true,
            identifier: Arc::new(mangled_name),
        })
    }

    fn gen_method(
        &mut self,
        class_name: &StrBuf,
        method_name: &StrBuf,
        method_descriptor: &MethodDescriptor,
        is_static: bool,
    ) -> Fallible<DeclIdentifier> {
        let mangled_name = mangle::mangle_method_name(
            class_name,
            method_name,
            &method_descriptor.ret,
            &method_descriptor.params,
        );

        let mut args = vec![];
        if !is_static {
            args.push(Type::Reference);
        }
        for ParameterDescriptor::Field(field_type) in method_descriptor.params.iter() {
            args.push(Type::from_field_type(field_type));
        }

        write!(
            self.out,
            "declare {return_type} @{mangled_name}(",
            return_type = tlt_return_type(&method_descriptor.ret),
            mangled_name = mangled_name
        )?;
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ")?;
            }
            write!(self.out, "{}", tlt_type(arg))?;
        }
        writeln!(self.out, ")")?;

        Ok(DeclIdentifier {
            global: true,
            identifier: Arc::new(mangled_name),
        })
    }

    fn gen_vtable_const(
        &mut self,
        class_name: &StrBuf,
        vtable_type: &DeclIdentifier,
    ) -> Fallible<DeclIdentifier> {
        let vtable_name = mangle::mangle_vtable_name(class_name);
        writeln!(
            self.out,
            "@{vtbl} = external constant {vtyp}",
            vtbl = vtable_name,
            vtyp = vtable_type
        )?;
        Ok(DeclIdentifier {
            global: true,
            identifier: Arc::new(vtable_name),
        })
    }

    fn gen_vtable_type(&mut self, class_name: &StrBuf) -> Fallible<DeclIdentifier> {
        let class_file = match self.classes.get(class_name)? {
            Class::File(class_file) => class_file,
            Class::Array(_) => bail!("can't generate vtable for array"),
        };
        let vtable = self.vtables.get(class_name)?;
        let vtable_name = mangle::mangle_vtable_name(class_name);
        writeln!(self.out, "%{} = type {{", vtable_name)?;
        if !class_file.is_interface() {
            writeln!(self.out, "  i32, ; <number of methods>")?;
        }
        for (idx, (key, _)) in vtable.iter_methods().enumerate() {
            writeln!(
                self.out,
                "  {} *, ; #{} method {}",
                GenFunctionType(&key.method_descriptor),
                idx,
                key.method_name
            )?;
        }
        write!(self.out, "  i32")?;
        if vtable.interface_count() > 0 {
            write!(self.out, ",")?;
        }
        writeln!(self.out, " ; <number of interfaces>")?;
        for (idx, (name, _)) in vtable.iter_interfaces().enumerate() {
            writeln!(self.out, "  i8*,")?;
            write!(self.out, "  i32")?;
            if idx < vtable.interface_count() - 1 {
                write!(self.out, ",")?;
            } else {
                write!(self.out, "")?;
            }
            writeln!(self.out, " ; #{} interface {}", idx, name)?;
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
