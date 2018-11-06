use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use classfile::ClassFile;
use classfile::descriptors::{BaseType,FieldType};
use failure::Fallible;
use jar::JarReader;

#[derive(Debug)]
pub(crate) enum Class {
    File(ClassFile),
    Array(ArrayClass),
}

#[derive(Debug)]
pub(crate) enum ArrayClass {
    Primitive(BaseType),
    Complex(Box<Class>)
}

pub(crate) trait ClassLoader {
    fn load(&self, name: &str) -> Fallible<Class>;
}

#[derive(Debug)]
pub(crate) struct BootstrapClassLoader {
    readers: Arc<Mutex<Vec<JarReader<File>>>>,
}

impl BootstrapClassLoader {
    pub fn open(paths: &[PathBuf]) -> Fallible<Self> {
        let mut readers = vec![];
        for path in paths {
            let file = File::open(path)?;
            let reader = JarReader::new(file)?;
            readers.push(reader);
        }
        Ok(Self {
            readers: Arc::new(Mutex::new(readers)),
        })
    }

    fn load_from_disk(&self, name: &str) -> Fallible<Class> {
        let mut readers = self.readers.lock().unwrap();
        for mut reader in readers.iter_mut() {
            if let Ok(class_file) = reader.get_class_file(name) {
                return Ok(Class::File(class_file));
            }
        }
        Err(format_err!("class {} not found", name))
    }

    fn load_array_by_component_type(&self, component_type: FieldType) -> Fallible<ArrayClass> {
        match component_type {
                FieldType::Base(base_type) => Ok(ArrayClass::Primitive(base_type)),
                FieldType::Array(array_type) => {
                    let inner = self.load_array_by_component_type(*array_type.component_type)?;
                    Ok(ArrayClass::Complex(Box::new(Class::Array(inner))))
                },
                FieldType::Object(object_type) => {
                    let class_name = object_type.class_name.replace(".", "/");
                    let class = self.load_from_disk(&class_name)?;
                    Ok(ArrayClass::Complex(Box::new(class)))
                }
            }
    }
}

impl ClassLoader for BootstrapClassLoader {
    fn load(&self, name: &str) -> Fallible<Class> {
        debug!("loading class {}", name);
        if name.starts_with('[') {
            let field_type = FieldType::try_from_str(&name[1..])?;
            let array_class = self.load_array_by_component_type(field_type)?;
            Ok(Class::Array(array_class))
        } else {
            self.load_from_disk(name)
        }
    }
}
