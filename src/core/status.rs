/// Typed FFI operation status codes conforming to RFP-027 F#61, F#62.
/// Stable numeric values organized into explicit semantic bands:
/// - 0: Ok (operation succeeded)
/// - 1..99: Safety & Lifecycle (null pointers, caught panics, ABI mismatch)
/// - 100..199: NotFound (state or entity does not exist)
/// - 200..299: InvalidArgument (malformed input, invalid pin/cursor, out-of-bounds limit)
/// - 300..399: BackendFailure (storage I/O, serialization fault, internal failure)
/// - 400..499: Unauthorized / Access Control (no grant, unknown caller, revoked grant)
/// - 500..599: Conflict (duplicate transactions, nonce mismatch, state conflict)
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum OperationStatus {
    #[default]
    Ok = 0,

    // Safety and Lifecycle (1..99)
    NullPointer = 1,
    PanicCaught = 2,
    Uninitialized = 3,
    AbiMismatch = 4,

    // Band 1: Not Found (100..199)
    NotFoundAccount = 100,
    NotFoundBlock = 101,
    NotFoundTransaction = 102,
    NotFoundCursor = 103,
    NotFoundGeneric = 104,

    // Band 2: Invalid Argument (200..299)
    InvalidArgumentCaller = 200,
    InvalidArgumentAccount = 201,
    InvalidArgumentCursor = 202,
    InvalidArgumentBlockPin = 203,
    InvalidArgumentLimit = 204,
    InvalidArgumentGeneric = 205,
    InvalidArgumentAbiMismatch = 206,

    // Band 3: Backend Failure (300..399)
    BackendFailureStorage = 300,
    BackendFailureSerialization = 301,
    BackendFailureLockPoisoned = 302,
    BackendFailureGeneric = 303,

    // Band 4: Unauthorized / Access Control (400..499)
    PermissionDenied = 400,
    UnauthorizedNoGrant = 401,
    UnauthorizedUnknownCaller = 402,
    UnauthorizedRevokedGrant = 403,

    // Band 5: Conflict (500..599)
    ConflictDuplicateTx = 500,
    ConflictNonce = 501,
    ConflictGeneric = 502,
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

    #[inline]
    pub const fn is_unauthorized(self) -> bool {
        let code = self as u32;
        code >= 400 && code < 500
    }

    #[inline]
    pub const fn is_conflict(self) -> bool {
        let code = self as u32;
        code >= 500 && code < 600
    }
}
