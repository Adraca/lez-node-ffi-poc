use crate::core::status::OperationStatus;
use std::ptr;

/// C-ABI compatible result container (RFP-027 F#58, F#61, F#62).
///
/// Encapsulates a pointer to the heap-allocated payload and a typed status code.
///
/// ### Ownership & Null Guarantee:
/// - When `status == OperationStatus::Ok`, `value` is guaranteed non-null and points to a valid,
///   heap-allocated payload owned by the caller. The caller is responsible for freeing it with the
///   type-paired deallocator (e.g. `lez_free_account`, `lez_free_tx_page`, etc.).
/// - When `status != OperationStatus::Ok`, `value` is strictly guaranteed to be `NULL` (`ptr::null_mut()`).
///   Callers do not need to free `value` on error and must not dereference it.
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
