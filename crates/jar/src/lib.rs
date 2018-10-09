pub extern crate classfile;
extern crate failure;
extern crate zip;

use std::fs;
use std::io::{Read, Seek};
use std::path::Path;

use classfile::ClassFile;
use failure::Fallible;
use zip::read::ZipArchive;

pub struct JarReader<R: Read + Seek> {
    archive: ZipArchive<R>,
}

impl<R: Read + Seek> JarReader<R> {
    pub fn new(reader: R) -> Fallible<Self> {
        let archive = ZipArchive::new(reader)?;
        Ok(JarReader { archive })
    }

    pub fn get_class_file(&mut self, name: &str) -> Fallible<ClassFile> {
        let entry = self.archive.by_name(&format!("{}.class", name))?;
        ClassFile::parse(entry)
    }
}

impl JarReader<fs::File> {
    pub fn open<P: AsRef<Path>>(path: P) -> Fallible<Self> {
        let file = fs::File::open(path)?;
        JarReader::new(file)
    }
}
