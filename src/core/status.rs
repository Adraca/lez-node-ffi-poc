/// Typed FFI operation status codes conforming to RFP-027 F#61, F#62.
/// Stable numeric values with an explicit 3-way error distinction:
/// - 100..199: NotFound (state or entity does not exist)
/// - 200..299: InvalidArgument (malformed input or invalid pin/token)
/// - 300..399: BackendFailure (storage I/O, poison, internal faults)
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub enum OperationStatus {
    #[default]
    Ok = 0,

    // Safety and Lifecycle (1..99)
    NullPointer = 1,
    PanicCaught = 2,
    Uninitialized = 3,
    PermissionDenied = 4,

    // 3-way Split 1: Not Found (100..199)
    NotFoundAccount = 100,
    NotFoundBlock = 101,
    NotFoundTransaction = 102,
    NotFoundCursor = 103,
    NotFoundGeneric = 104,

    // 3-way Split 2: Invalid Argument (200..299)
    InvalidArgumentCaller = 200,
    InvalidArgumentAccount = 201,
    InvalidArgumentCursor = 202,
    InvalidArgumentBlockPin = 203,
    InvalidArgumentLimit = 204,
    InvalidArgumentGeneric = 205,

    // 3-way Split 3: Backend Failure (300..399)
    BackendFailureStorage = 300,
    BackendFailureSerialization = 301,
    BackendFailureLockPoisoned = 302,
    BackendFailureGeneric = 303,
}

impl OperationStatus {
    #[inline]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    #[inline]
    pub const fn is_not_found(self) -> bool {
        let code = self as u32;
        code >= 100 && code < 200
    }

    #[inline]
    pub const fn is_invalid_argument(self) -> bool {
        let code = self as u32;
        code >= 200 && code < 300
    }

    #[inline]
    pub const fn is_backend_failure(self) -> bool {
        let code = self as u32;
        code >= 300 && code < 400
    }
}
