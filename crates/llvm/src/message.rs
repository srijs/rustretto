use std::ffi::CStr;
use std::fmt;

use libc::c_char;
use llvm_sys::core::*;

pub struct Message {
    pub(crate) inner: *mut c_char,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cow_str = unsafe { CStr::from_ptr(self.inner).to_string_lossy() };
        f.write_str(&*cow_str)
    }
}

impl Drop for Message {
    fn drop(&mut self) {
        unsafe {
            LLVMDisposeMessage(self.inner);
        }
    }
}
