use std::ptr;
use crate::core::status::OperationStatus;

/// C-ABI compatible result container (RFP-027 F#58, F#61, F#62).
///
/// Encapsulates a pointer to the heap-allocated payload and a typed
/// status code. The pointer is guaranteed valid only when `status == OperationStatus::Ok`.
#[repr(C)]
pub struct PointerResult<T> {
    pub value: *mut T,
    pub status: OperationStatus,
}

impl<T> PointerResult<T> {
    #[inline]
    pub fn from_value(value: T) -> Self {
        Self {
            value: Box::into_raw(Box::new(value)),
            status: OperationStatus::Ok,
        }
    }

    #[inline]
    pub const fn from_error(status: OperationStatus) -> Self {
        Self {
            value: ptr::null_mut(),
            status,
        }
    }

    #[inline]
    pub const fn null_pointer() -> Self {
        Self::from_error(OperationStatus::NullPointer)
    }

    #[inline]
    pub const fn panic_caught() -> Self {
        Self::from_error(OperationStatus::PanicCaught)
    }
}

/// Frees an arbitrary `PointerResult<T>` wrapper and drops the boxed payload.
///
/// # Safety
/// If `ptr` is non-null, it must have been allocated via `Box::into_raw` by this crate
/// and not previously freed.
pub unsafe fn free_boxed_pointer<T>(ptr: *mut T) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}
