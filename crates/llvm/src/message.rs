use libc::c_char;
use llvm_sys::core::*;

pub struct Message {
    pub(crate) inner: *mut c_char,
}

impl Drop for Message {
    fn drop(&mut self) {
        unsafe {
            LLVMDisposeMessage(self.inner);
        }
    }
}
