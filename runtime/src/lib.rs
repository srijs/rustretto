extern crate libc;

use libc::c_void;
//mod unwind;

#[repr(C)]
pub struct Ref {
    object: *const c_void,
    vtable: *const c_void,
}

#[no_mangle]
pub unsafe extern "C" fn _Jm_java_lang_Object__init(_this: Ref) {}
