use crate::bindings::ForeignInvokeResult;
use groundhog_xdr::ffi_enums::InvokeErrorCode;
use std::error::Error;
use std::fmt;
use std::panic;
use std::panic::panic_any;
use std::panic::UnwindSafe;

impl PartialEq for ForeignInvokeResult {
    fn eq(&self, other: &Self) -> bool {
        (self.result_code == other.result_code) && (self.panic == other.panic)
    }
}

#[derive(Debug, Clone, PartialEq)]
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
        error_type: InvokeErrorCode::kInvokeDeterministic,
        error_code: 0,
    }
}

pub fn out_of_gas_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeOutOfGas,
        error_code: 0,
    }
}

pub fn unrecoverable_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeUnrecoverable,
        error_code: 0,
    }
}

pub fn exit_error(code: i32) -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeNone,
        error_code: code,
    }
}

pub fn return_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeNone,
        error_code: 0,
    }
}

pub fn validation_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::kInvokeValidationFailure,
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

// Wrap a function (i.e. part of a contract) within a try-catch block
// that appropriately handles groundhog error codes.
pub fn invoke_wrapper<'a, F>(
    wrapped: F,
) -> Box<dyn Fn() -> ForeignInvokeResult + 'a>
where
    F: Fn() -> i32 + UnwindSafe + Clone + 'a,
{
    Box::new(move || {
        let w = wrapped.clone();
        let result = panic::catch_unwind(move || w());

        match result {
            Ok(result_code) => {
                return ForeignInvokeResult {
                    result_code: result_code,
                    panic: InvokeErrorCode::kInvokeNone as i32,
                }
            }
            Err(err) => {
                if let Some(my_error) = err.downcast_ref::<InvokeError>() {
                    return ForeignInvokeResult {
                        result_code: my_error.error_code,
                        panic: my_error.error_type.clone() as i32,
                    };
                }
                return ForeignInvokeResult {
                    result_code: 0,
                    panic: InvokeErrorCode::kInvokeUnrecoverable as i32,
                };
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // There's a race condition on the panic handler,
    // which changes how tests behave.  So run them all sequentially.
    #[test]
    fn test_panic_runner() {
        test_error_wrap();
        test_panic();
        test_panic_ok();
        test_sdk_panic();
        test_option_panic();
        test_pass_invoke_error_through_panic_hook();
    }

    fn test_error_wrap() {
        let _ = panic::take_hook();
        let f: fn() -> i32 = || {
            return 1;
        };

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            ForeignInvokeResult {
                result_code: 1,
                panic: 0
            }
        );
    }

    fn test_panic() {
        let _ = panic::take_hook();
        let f: fn() -> i32 = || panic!("unknown");

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            ForeignInvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kInvokeUnrecoverable as i32
            }
        );
    }

    fn test_panic_ok() {
        let _ = panic::take_hook();
        let f: fn() -> i32 =
            || panic_any(crate::invoke_error::validation_error());

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            ForeignInvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kInvokeValidationFailure as i32
            }
        );
    }

    fn test_sdk_panic() {
        set_override_panic_hook();
        let f: fn() -> i32 = || {
            panic!("something");
        };

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            ForeignInvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kInvokeDeterministic as i32
            }
        );
    }

    fn test_option_panic() {
        set_override_panic_hook();
        let f: fn() -> i32 = || {
            let x: Option<i32> = None;
            return x.unwrap();
        };

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            ForeignInvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kInvokeDeterministic as i32
            }
        );
    }

    fn test_pass_invoke_error_through_panic_hook() {
        set_override_panic_hook();
        let f: fn() -> i32 =
            || panic_any(crate::invoke_error::validation_error());

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            ForeignInvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kInvokeValidationFailure as i32
            }
        );
    }
}
