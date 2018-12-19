use libc::c_char;

use super::Ref;

#[repr(C)]
pub(crate) struct VTablePrintStream {
    padding: [usize; 40],
    println_string: unsafe extern "C" fn(Ref, Ref),
}

pub(crate) static VTABLE_PRINTSTREAM: &VTablePrintStream = &VTablePrintStream {
    padding: [0; 40],
    println_string: printstream_println_string_stub,
};

unsafe extern "C" fn printstream_println_string_stub(_this: Ref, string: Ref) {
    libc::puts(string.object as *const c_char);
}
