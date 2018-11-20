extern crate libc;

use libc::{c_void, c_char};

#[repr(C)]
pub struct Ref {
    object: *const c_void,
    vtable: *const c_void,
}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object__init(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _Jf_java_lang_System_out__get(_this: Ref) -> Ref {
    Ref {
        object: std::ptr::null(),
        vtable: std::ptr::null(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_io_PrintStream_println__Ljava_lang_String_2(
    _this: Ref,
    string: Ref,
) {
    libc::puts(string.object as *const c_char);
}

#[no_mangle]
pub unsafe extern "C" fn _Jrt_ldstr(_len: i32, bytes: *const i8) -> Ref {
    // horrible hack! :(
    Ref {
        object: bytes as *const c_void,
        vtable: std::ptr::null(),
    }
}
