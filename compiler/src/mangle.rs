use std::fmt::Write;
use std::hash::{Hash, Hasher};

use classfile::descriptors::{BaseType, FieldType, ParameterDescriptor, ReturnTypeDescriptor};
use fnv::FnvHasher;

pub fn mangle_field_name_setter(class_name: &str, field_name: &str) -> String {
    mangle_field_accessor(class_name, field_name, true)
}

pub fn mangle_field_name_getter(class_name: &str, field_name: &str) -> String {
    mangle_field_accessor(class_name, field_name, false)
}

pub fn mangle_method_name(
    class_name: &str,
    method_name: &str,
    rettype: &ReturnTypeDescriptor,
    params: &[ParameterDescriptor],
) -> String {
    let mut mangler = Mangler::new("Z");

    mangler.nested_start();

    for ns in class_name.split("/") {
        mangler.name(&ns);
    }

    match method_name {
        "<init>" => mangler.output.push_str("4init"),
        "<clinit>" => mangler.output.push_str("6clinit"),
        _ => mangler.name(method_name),
    }

    let mut hasher = FnvHasher::default();
    (class_name, method_name, rettype, rettype).hash(&mut hasher);
    let hash = hasher.finish();
    write!(mangler.output, "Iu9J{:08x}E", hash as u32).unwrap();

    mangler.nested_end();

    match rettype {
        ReturnTypeDescriptor::Void => mangler.output.push('v'),
        ReturnTypeDescriptor::Field(ref field_type) => mangler.field_type(field_type),
    };

    if params.is_empty() {
        mangler.output.push('v');
    } else {
        for ParameterDescriptor::Field(field_type) in params {
            mangler.field_type(&field_type);
        }
    }

    return mangler.output;
}

fn mangle_field_accessor(class_name: &str, field_name: &str, setter: bool) -> String {
    let mut mangler = Mangler::new("Z");

    mangler.nested_start();

    for ns in class_name.split("/") {
        mangler.name(&ns);
    }

    mangler.name(field_name);

    if setter {
        mangler.output.push_str("v13set");
    } else {
        mangler.output.push_str("v03get");
    }

    mangler.nested_end();

    return mangler.output;
}

pub fn mangle_vtable_name(class_name: &str) -> String {
    format!("vtable.{}", mangle(class_name))
}

fn mangle(input: &str) -> String {
    let mut output = input.to_owned();
    output = output.replace("_", "_1");
    output = output.replace(";", "_2");
    output = output.replace("[", "_3");
    output = output.replace("/", "_");
    output = output.replace(".", "_");
    return output;
}

struct Mangler {
    output: String,
}

impl Mangler {
    fn new(prefix: &str) -> Self {
        Mangler {
            output: format!("_{}", prefix),
        }
    }

    fn nested_start(&mut self) {
        self.output.push('N');
    }

    fn nested_end(&mut self) {
        self.output.push('E');
    }

    fn name(&mut self, name: &str) {
        write!(self.output, "{}{}", name.len(), name).unwrap();
    }

    fn field_type(&mut self, mut field_type: &FieldType) {
        loop {
            match field_type {
                FieldType::Base(base_type) => {
                    match base_type {
                        BaseType::Byte => self.output.push_str("u4byte"),
                        BaseType::Char => self.output.push_str("u4char"),
                        BaseType::Double => self.output.push('d'),
                        BaseType::Float => self.output.push('f'),
                        BaseType::Int => self.output.push('i'),
                        BaseType::Long => self.output.push('l'),
                        BaseType::Short => self.output.push('s'),
                        BaseType::Boolean => self.output.push_str("u7boolean"),
                    };
                    break;
                }
                FieldType::Object(object_type) => {
                    self.nested_start();
                    for ns in object_type.class_name.split(".") {
                        self.name(&ns);
                    }
                    self.nested_end();
                    break;
                }
                FieldType::Array(array_type) => {
                    self.output.push_str("A_");
                    field_type = &*array_type.component_type;
                }
            }
        }
    }
}
