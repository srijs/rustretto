use std::ffi::CStr;

#[derive(Debug)]
pub struct Error {
    pub(crate) message: String,
}

impl Error {
    pub(crate) unsafe fn from_ptr(ptr: *const libc::c_char) -> Self {
        let message = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        Error { message }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}
