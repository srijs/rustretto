pub extern crate classfile;

use std::fs;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use bytes::Bytes;
use classfile::ClassFile;
use failure::Fallible;
use fnv::FnvBuildHasher;
use zip::read::ZipArchive;

mod manifest;
pub use self::manifest::Manifest;

#[derive(Debug)]
pub struct JarReader<R: Read + Seek> {
    archive: ZipArchive<BufReader<R>, FnvBuildHasher>,
    manifest: Option<Manifest>,
}

impl<R: Read + Seek> JarReader<R> {
    pub fn try_new(reader: R) -> Fallible<Self> {
        let mut archive =
            ZipArchive::new_with_hasher(BufReader::new(reader), FnvBuildHasher::default())?;

        let manifest = match archive.by_name("META-INF/MANIFEST.MF") {
            Ok(file) => Some(Manifest::parse(file)?),
            Err(zip::result::ZipError::FileNotFound) => None,
            Err(err) => return Err(err.into()),
        };

        Ok(JarReader { manifest, archive })
    }

    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref()
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
        JarReader::try_new(file)
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
