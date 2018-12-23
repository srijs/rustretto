use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};

use classfile::descriptors::{BaseType, FieldType};
use classfile::ClassFile;
use failure::{format_err, Fallible};
use jar::{ClassEntry, JarReader};
use strbuf::StrBuf;

#[derive(Clone, Debug)]
pub(crate) enum Class {
    File(Arc<ClassFile>),
    Array(ArrayClass),
}

#[derive(Clone, Debug)]
pub(crate) enum ArrayClass {
    Primitive(BaseType),
    Complex(Box<Class>),
}

pub(crate) trait ClassLoader {
    fn load(&self, name: &str) -> Fallible<Class>;
}

#[derive(Clone, Debug)]
pub(crate) struct InputClassLoader<P> {
    inputs: HashMap<StrBuf, Arc<ClassFile>>,
    parent: P,
}

impl<P> InputClassLoader<P> {
    pub fn new(parent: P) -> Self {
        Self {
            inputs: HashMap::new(),
            parent,
        }
    }

    pub fn add_input(&mut self, class_file: ClassFile) {
        let name = class_file.get_name().clone();
        self.inputs.insert(name, Arc::new(class_file));
    }
}

impl<P> ClassLoader for InputClassLoader<P>
where
    P: ClassLoader,
{
    fn load(&self, name: &str) -> Fallible<Class> {
        if let Some(class_file) = self.inputs.get(name) {
            return Ok(Class::File(class_file.clone()));
        }
        self.parent.load(name)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BootstrapClassLoader {
    readers: Arc<Mutex<Vec<JarReader<File>>>>,
}

impl BootstrapClassLoader {
    pub fn open<P: AsRef<Path>>(home: P) -> Fallible<Self> {
        let paths = &[
            home.as_ref().join("jre/lib/rt.jar"),
            home.as_ref().join("jre/lib/jce.jar"),
        ];

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

    fn load_entry_from_disk(&self, name: &str) -> Fallible<ClassEntry> {
        let mut readers = self.readers.lock().unwrap();
        for reader in readers.iter_mut() {
            if let Ok(class_entry) = reader.get_class_entry(name) {
                return Ok(class_entry);
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
            }
            FieldType::Object(object_type) => {
                let class_name = object_type.class_name.replace(".", "/");
                let class_entry = self.load_entry_from_disk(&class_name)?;
                let class_file = class_entry.decode()?;
                let class = Class::File(Arc::new(class_file));
                Ok(ArrayClass::Complex(Box::new(class)))
            }
        }
    }
}

impl ClassLoader for BootstrapClassLoader {
    fn load(&self, name: &str) -> Fallible<Class> {
        log::debug!("loading class {}", name);
        if name.starts_with('[') {
            let field_type = FieldType::try_from_str(&name[1..])?;
            let array_class = self.load_array_by_component_type(field_type)?;
            Ok(Class::Array(array_class))
        } else {
            let class_entry = self.load_entry_from_disk(name)?;
            let class_file = class_entry.decode()?;
            Ok(Class::File(Arc::new(class_file)))
        }
    }
}
