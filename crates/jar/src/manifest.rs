use std::io::Read;

use failure::{ensure, Fallible};
use indexmap::IndexMap;
use strbuf::StrBuf;

#[derive(Debug)]
pub struct Manifest {
    main: IndexMap<StrBuf, StrBuf>,
}

impl Manifest {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.main.get(name).map(|value| &*value as &str)
    }

    pub(crate) fn parse<R>(mut read: R) -> Fallible<Self>
    where
        R: Read,
    {
        let mut buf = String::new();
        read.read_to_string(&mut buf)?;
        let strbuf = StrBuf::from(buf);

        let mut main = IndexMap::new();

        for line in strbuf.lines() {
            // skip empty lines
            if line.is_empty() {
                continue;
            }

            // parse header name
            let name_start_idx = 0;
            let mut name_end_idx = name_start_idx;
            for (idx, c) in line.char_indices() {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    name_end_idx = name_start_idx + idx;
                } else {
                    break;
                }
            }

            // parse header delimiter
            ensure!(
                &line[name_end_idx + 1..=name_end_idx + 2] == ": ",
                "bad delimiter"
            );

            // parse header value
            let value_start_idx = name_end_idx + 3;
            let mut value_end_idx = value_start_idx;
            for (idx, c) in line[value_start_idx..].char_indices() {
                if c != '\0' || c == '\r' || c == '\n' {
                    value_end_idx = value_start_idx + idx;
                } else {
                    break;
                }
            }

            let name = strbuf.str_ref(&line[name_start_idx..=name_end_idx]);
            let value = strbuf.str_ref(&line[value_start_idx..=value_end_idx]);
            main.insert(name, value);
        }

        Ok(Manifest { main })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        let input = "Manifest-Version: 1.0\nCreated-By: 1.8.0_181 (Oracle Corporation)\nMain-Class: Test\n\n";
        let manifest = Manifest::parse(std::io::Cursor::new(input)).unwrap();

        assert_eq!("1.0", manifest.get("Manifest-Version").unwrap());
        assert_eq!(
            "1.8.0_181 (Oracle Corporation)",
            manifest.get("Created-By").unwrap()
        );
        assert_eq!("Test", manifest.get("Main-Class").unwrap());
    }
}
