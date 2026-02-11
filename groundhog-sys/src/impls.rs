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
    #[error("Event log not found")]
    EventLogNotFound =
        crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeEventLogNotFound,
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
            x if x == crate::bindings::TestFFIErrorCode_kTestFFIErrorCodeEventLogNotFound => FFIErrorCode::EventLogNotFound,
            _ => panic!("unknown FFI error code: {}", error),
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
