use hogexec_internal::ffi_enums::InvokeErrorCode;
use std::error::Error;
use std::fmt;
use std::panic;
use std::panic::UnwindSafe;
use std::panic::panic_any;

#[derive(Debug, Clone, PartialEq)]
pub struct InvokeError {
    pub error_type: InvokeErrorCode,
}

impl fmt::Display for InvokeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InvokeError {}", self.clone())
    }
}

impl Error for InvokeError {}

pub fn deterministic_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeDeterministic,
    }
}

pub fn out_of_gas_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeOutOfGas,
    }
}

pub fn unrecoverable_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeUnrecoverable,
    }
}

pub fn exit_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeNone,
    }
}

pub fn return_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeNone,
    }
}

pub fn validation_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeValidationFailure,
    }
}

pub fn abort_transaction_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeAbortTransaction,
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

pub fn clear_override_panic_hook() {
    let _ = panic::take_hook();
}

// Wrap a function (i.e. part of a contract) within a try-catch block
// that appropriately handles groundhog error codes.
pub fn invoke_wrapper<'a, F>(
    wrapped: F,
) -> Box<dyn Fn() -> groundhog_sys::ForeignInvokeResult + 'a>
where
    F: Fn() + UnwindSafe + Clone + 'a,
{
    Box::new(move || {
        let w = wrapped.clone();
        let result = panic::catch_unwind(w);

        match result {
            Ok(_) => groundhog_sys::ForeignInvokeResult {
                panic: InvokeErrorCode::kInvokeNone as i32,
            },
            Err(err) => {
                if let Some(my_error) = err.downcast_ref::<InvokeError>() {
                    return groundhog_sys::ForeignInvokeResult {
                        panic: my_error.error_type as i32,
                    };
                }
                groundhog_sys::ForeignInvokeResult {
                    panic: InvokeErrorCode::kInvokeUnrecoverable as i32,
                }
            }
        }
    })
}
