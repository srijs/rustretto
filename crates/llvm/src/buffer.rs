use std::ops::Deref;
use std::slice;

use llvm_sys::core::*;
use llvm_sys::prelude::*;

pub struct MemoryBuffer {
    pub(crate) llref: LLVMMemoryBufferRef,
}

impl Deref for MemoryBuffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe {
            let ptr = LLVMGetBufferStart(self.llref) as *const u8;
            let len = LLVMGetBufferSize(self.llref);
            slice::from_raw_parts(ptr, len)
        }
    }
}

impl Drop for MemoryBuffer {
    fn drop(&mut self) {
        unsafe {
            LLVMDisposeMemoryBuffer(self.llref);
        }
    }
}
