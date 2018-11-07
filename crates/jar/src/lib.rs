pub extern crate classfile;

extern crate bytes;
extern crate failure;
extern crate zip;

use std::fs;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use bytes::Bytes;
use classfile::ClassFile;
use failure::Fallible;
use zip::read::ZipArchive;

#[derive(Debug)]
pub struct JarReader<R: Read + Seek> {
    archive: ZipArchive<BufReader<R>>,
}

impl<R: Read + Seek> JarReader<R> {
    pub fn new(reader: R) -> Fallible<Self> {
        let archive = ZipArchive::new(BufReader::new(reader))?;
        Ok(JarReader { archive })
    }

    pub fn get_class_entry(&mut self, name: &str) -> Fallible<ClassEntry> {
        let mut file = self.archive.by_name(&format!("{}.class", name))?;
        let mut data = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut data)?;
        Ok(ClassEntry { bytes: data.into() })
    }
}

impl JarReader<fs::File> {
    pub fn open<P: AsRef<Path>>(path: P) -> Fallible<Self> {
        let file = fs::File::open(path)?;
        JarReader::new(file)
    }
}

#[derive(Clone, Debug)]
pub struct ClassEntry {
    bytes: Bytes,
}

impl ClassEntry {
    pub fn decode(&self) -> Fallible<ClassFile> {
        ClassFile::parse_bytes(self.bytes.clone())
    }
}
