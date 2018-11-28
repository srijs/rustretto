use libc::c_char;

use super::Ref;

#[repr(C)]
pub(crate) struct VTablePrintStream {
    padding: [usize; 40],
    println_string: unsafe extern "C" fn(Ref, Ref),
}

pub(crate) static VTABLE_PRINTSTREAM: &VTablePrintStream = &VTablePrintStream {
    padding: [0; 40],
    println_string: _Jm_java_io_PrintStream_println__Z__Ljava_lang_String_2,
};

#[no_mangle]
unsafe extern "C" fn _Jm_java_io_PrintStream_println__Z__Ljava_lang_String_2(
    _this: Ref,
    string: Ref,
) {
    libc::puts(string.object as *const c_char);
}
