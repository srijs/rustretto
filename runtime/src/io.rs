use super::Ref;

#[repr(C)]
pub(crate) struct VTablePrintStream {
    padding: [usize; 40],
    println_string: unsafe extern "C" fn(Ref, Ref),
}

pub(crate) static VTABLE_PRINTSTREAM: &VTablePrintStream = &VTablePrintStream {
    padding: [0; 40],
    println_string: ::stubs::_Jm_java_io_PrintStream_println__Z__Ljava_lang_String_2,
};
