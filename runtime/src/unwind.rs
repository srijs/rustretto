use libc::{c_void, uint32_t, uint64_t, uintptr_t};

const EXCEPTION_CLASS: uint64_t = 0x4A415641;

#[no_mangle]
pub unsafe extern "C" fn _Jn_exception_throw(object: *const c_void) {
    let mut exception_object = _Unwind_Exception {
        exception_class: EXCEPTION_CLASS,
        exception_cleanup: _Jn_exception_unwind_cleanup,
        private_1: 0,
        private_2: 0,
        reserved: [0; 3],
    };
    _Unwind_RaiseException(&mut exception_object as *mut _Unwind_Exception);
}

#[no_mangle]
unsafe extern "C" fn _Jn_exception_unwind_cleanup(
    reason: _Unwind_Reason_Code,
    exception_object: *mut _Unwind_Exception,
) {

}

#[repr(C)]
#[allow(non_camel_case_types)]
enum _Unwind_Reason_Code {
    _URC_NO_REASON = 0,
    _URC_FOREIGN_EXCEPTION_CAUGHT = 1,
    _URC_FATAL_PHASE2_ERROR = 2,
    _URC_FATAL_PHASE1_ERROR = 3,
    _URC_NORMAL_STOP = 4,
    _URC_END_OF_STACK = 5,
    _URC_HANDLER_FOUND = 6,
    _URC_INSTALL_CONTEXT = 7,
    _URC_CONTINUE_UNWIND = 8,
}

#[allow(non_camel_case_types)]
type _Unwind_Exception_Cleanup_Fn =
    unsafe extern "C" fn(_Unwind_Reason_Code, *mut _Unwind_Exception);

#[repr(C)]
struct _Unwind_Exception {
    exception_class: uint64_t,
    exception_cleanup: _Unwind_Exception_Cleanup_Fn,
    private_1: uint64_t,
    private_2: uint64_t,
    reserved: [uint32_t; 3],
}

#[repr(C)]
pub struct _Unwind_Context {
    private: [u8; 0],
}

extern "C" {
    fn _Unwind_RaiseException(exception_object: *mut _Unwind_Exception) -> _Unwind_Reason_Code;
    fn _Unwind_DeleteException(exception_object: *mut _Unwind_Exception);

    fn _Unwind_GetIP(context: *mut _Unwind_Context) -> uintptr_t;

    fn _Unwind_GetLanguageSpecificData(context: *mut _Unwind_Context) -> uint64_t;
}
