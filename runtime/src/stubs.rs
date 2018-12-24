use std::ptr;

use libc::c_void;

use crate::Ref;

#[no_mangle]
pub static _ZN4java4lang6System3outE: Ref = Ref {
    object: ptr::null(),
    vtable: crate::io::VTABLE_PRINTSTREAM as *const crate::io::VTablePrintStream as *const c_void,
};

#[no_mangle]
pub static _ZTVN4java4lang13StringBuilderE: () = ();

#[no_mangle]
pub static _ZTVN4java4lang24IllegalArgumentExceptionE: () = ();

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang13StringBuilder4initIu9J8cc45093EEvv(_this: Ref) {}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang24IllegalArgumentException4initIu9Jffb6fc97EEvN4java4lang6StringE(
    _this: Ref,
    _string: Ref,
) {
}

#[no_mangle]
pub unsafe extern "C" fn _ZN4java4lang7Integer11toHexStringIu9Jab2e85aaEEN4java4lang6StringEi(
    _value: i64,
) -> Ref {
    Ref::null()
}
