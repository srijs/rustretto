use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

use bytes::Bytes;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StrBuf(string::String<Bytes>);

impl StrBuf {
    pub fn from_str(s: &str) -> Self {
        StrBuf(string::String::from_str(s))
    }

    pub unsafe fn from_utf8_unchecked(bytes: Bytes) -> Self {
        StrBuf(string::String::from_utf8_unchecked(bytes))
    }

    pub fn str_ref(&self, s: &str) -> Self {
        let bytes = self.0.get_ref().slice_ref(s.as_bytes());
        unsafe { StrBuf::from_utf8_unchecked(bytes) }
    }
}

impl From<String> for StrBuf {
    fn from(string: String) -> Self {
        let bytes = string.into();
        // SAFETY: We know that the bytes contain valid utf8, since
        //         they have been converted from the input string.
        unsafe { Self::from_utf8_unchecked(bytes) }
    }
}

impl Hash for StrBuf {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Because of the impl Borrow<str> for StrBuf, we need to make sure that the Hash
        // implementations behave identically between str and StrBuf.
        str::hash(&*self, state)
    }
}

impl Borrow<str> for StrBuf {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}

impl fmt::Display for StrBuf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&*self.0)
    }
}

impl Deref for StrBuf {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}
