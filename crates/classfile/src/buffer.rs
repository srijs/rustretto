use std::borrow::Cow;
use std::io::Read;
use std::ops::Deref;

use bytes::{Buf, Bytes};
use cesu8;
use failure::Fallible;
use string;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StrBuf(string::String<Bytes>);

impl StrBuf {
    pub(crate) fn from_str(s: &str) -> Self {
        StrBuf(string::String::from_str(s))
    }

    pub(crate) fn from_java_cesu8(bytes: &ByteBuf) -> Fallible<Self> {
        let s = match cesu8::from_java_cesu8(&bytes.0)? {
            Cow::Owned(s) => {
                // SAFETY: We convert a `String` into `Bytes`, which means that
                //         the byte buffer only contains valid UTF-8.
                unsafe { string::String::from_utf8_unchecked(s.into()) }
            }
            Cow::Borrowed(_) => {
                // SAFETY: The `cesu8::from_java_cesu8` function has successfully
                //         returned a borrowed string, therefore we conclude that
                //         the input buffer is valid UTF-8.
                unsafe { string::String::from_utf8_unchecked(bytes.0.clone()) }
            }
        };
        Ok(StrBuf(s))
    }
}

impl Deref for StrBuf {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ByteBuf(Bytes);

impl ByteBuf {
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn split_to(&mut self, at: usize) -> ByteBuf {
        ByteBuf(self.0.split_to(at))
    }
}

impl Buf for ByteBuf {
    fn remaining(&self) -> usize {
        self.0.len()
    }

    fn bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    fn advance(&mut self, cnt: usize) {
        self.0.advance(cnt)
    }
}

impl AsRef<[u8]> for ByteBuf {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Read for ByteBuf {
    fn read(&mut self, buf: &mut [u8]) -> ::std::io::Result<usize> {
        self.reader().read(buf)
    }
}

impl From<Vec<u8>> for ByteBuf {
    fn from(vec: Vec<u8>) -> Self {
        ByteBuf(vec.into())
    }
}

impl From<Bytes> for ByteBuf {
    fn from(bytes: Bytes) -> Self {
        ByteBuf(bytes)
    }
}
