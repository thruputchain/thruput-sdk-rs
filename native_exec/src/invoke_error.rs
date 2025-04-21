use groundhog_xdr::ffi_enums::InvokeErrorCode;

use std::error::Error;
use std::fmt;
use std::panic;
use std::panic::panic_any;

#[derive(Debug, Clone)]
pub struct InvokeError {
    pub error_type: InvokeErrorCode,
    pub error_code: i32,
}

impl fmt::Display for InvokeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InvokeError {}", self.clone())
    }
}

impl Error for InvokeError {}

pub fn deterministic_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kDeterministic,
        error_code: 0,
    }
}

pub fn out_of_gas_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kOutOfGas,
        error_code: 0,
    }
}

pub fn unrecoverable_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kUnrecoverable,
        error_code: 0,
    }
}

pub fn exit_error(code: i32) -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kNone,
        error_code: code,
    }
}

pub fn return_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kNone,
        error_code: 0,
    }
}

pub fn validation_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kValidationFailure,
        error_code: 0,
    }
}

pub fn trap() -> ! {
    panic_any(Box::new(deterministic_error()))
}

pub fn set_override_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        if let Some(payload) =
            panic_info.payload().downcast_ref::<InvokeError>()
        {
            panic::resume_unwind(Box::new(payload.clone()));
        }

        if let Some(payload) = panic_info.payload().downcast_ref::<String>() {
            eprintln!("Panic message: {}", payload);
        } else if let Some(payload) =
            panic_info.payload().downcast_ref::<&str>()
        {
            eprintln!("Panic message: {}", payload);
        } else {
            eprintln!("Panic: {:?}", panic_info.payload());
        }

        if let Some(location) = panic_info.location() {
            eprintln!(
                "Panic occurred at file '{}' line {}",
                location.file(),
                location.line()
            );
        }

        panic::resume_unwind(Box::new(deterministic_error()));
    }));
}