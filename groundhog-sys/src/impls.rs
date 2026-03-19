#[derive(thiserror::Error, Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
pub enum FFIErrorCode {
    #[error("No error")]
    None = crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeNone,
    #[error("Invalid pointer")]
    InvalidPointer =
        crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeInvalidPointer,
    #[error(
        "Cannot deploy a contract that has not yet been created (either by a transaction in an earlier block, or by the setup_create_contract function)"
    )]
    DeployNonexistentContract = crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeDeployNonexistentContract,
    #[error(
        "Cannot create a mock contract for a mock contract ID that has not yet been registered (with add_contract)"
    )]
    CreateNonexistentMockContract = crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeCreateNonexistentMockContract,
    #[error(
        "Supplied data structs failed to deserialize into expected format"
    )]
    DeserializeError =
        crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeDeserializeError,
    #[error("Unrecoverable error")]
    UnrecoverableError =
        crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeUnrecoverableError,
}

impl From<crate::bindings::TestFFIErrorCode> for FFIErrorCode {
    fn from(error: crate::bindings::TestFFIErrorCode) -> Self {
        match error {
            x if x == crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeNone => FFIErrorCode::None,
            x if x == crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeInvalidPointer => FFIErrorCode::InvalidPointer,
            x if x == crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeDeployNonexistentContract => FFIErrorCode::DeployNonexistentContract,
            x if x == crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeCreateNonexistentMockContract => FFIErrorCode::CreateNonexistentMockContract,
            x if x == crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeDeserializeError => FFIErrorCode::DeserializeError,
            x if x == crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeUnrecoverableError => FFIErrorCode::UnrecoverableError,
            _ => panic!("unknown FFI error code: {}", error),
        }
    }
}

#[derive(thiserror::Error, Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
pub enum FFILogLevel {
    #[error("Trace logging")]
    Trace = crate::bindings::TestFFILogLevel_kTestFFILogLevelTrace,
    #[error("Debug logging")]
    Debug = crate::bindings::TestFFILogLevel_kTestFFILogLevelDebug,
    #[error("Info logging")]
    Info = crate::bindings::TestFFILogLevel_kTestFFILogLevelInfo,
    #[error("Warn logging")]
    Warn = crate::bindings::TestFFILogLevel_kTestFFILogLevelWarn,
    #[error("Error logging")]
    Error = crate::bindings::TestFFILogLevel_kTestFFILogLevelError,
    #[error("Critical logging")]
    Critical = crate::bindings::TestFFILogLevel_kTestFFILogLevelCritical,
}

impl From<crate::bindings::TestFFILogLevel> for FFILogLevel {
    fn from(level: crate::bindings::TestFFILogLevel) -> Self {
        match level {
            x if x == crate::bindings::TestFFILogLevel_kTestFFILogLevelTrace => FFILogLevel::Trace,
            x if x == crate::bindings::TestFFILogLevel_kTestFFILogLevelDebug => FFILogLevel::Debug,
            x if x == crate::bindings::TestFFILogLevel_kTestFFILogLevelInfo => FFILogLevel::Info,
            x if x == crate::bindings::TestFFILogLevel_kTestFFILogLevelWarn => FFILogLevel::Warn,
            x if x == crate::bindings::TestFFILogLevel_kTestFFILogLevelError => FFILogLevel::Error,
            x if x == crate::bindings::TestFFILogLevel_kTestFFILogLevelCritical => FFILogLevel::Critical,
            _ => panic!("unknown FFI log level: {}", level),
        }
    }
}

impl Drop for crate::FFIDataBuffer {
    fn drop(&mut self) {
        match unsafe {
            crate::bindings::free_ffi_data_buffer(core::ptr::read(self))
        } {
            crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeNone => (),
            error => panic!("failed to free FFI data buffer: {}", error),
        }
    }
}

impl crate::FFIDataBuffer {
    pub fn as_slice(&self) -> Option<&[u8]> {
        if self.len == 0 {
            return None;
        }
        Some(unsafe {
            std::slice::from_raw_parts(self.data, self.len.try_into().ok()?)
        })
    }
}
