use hogexec_internal::ffi_enums::InvokeErrorCode;
use std::cell::Cell;
use std::error::Error;
use std::fmt;
use std::panic;
use std::panic::UnwindSafe;
use std::panic::panic_any;
use std::sync::Once;

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
        error_type: InvokeErrorCode::Deterministic,
    }
}

pub fn out_of_gas_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::OutOfGas,
    }
}

pub fn unrecoverable_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::Unrecoverable,
    }
}

pub fn exit_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::None,
    }
}

pub fn return_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::None,
    }
}

pub fn validation_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::ValidationFailure,
    }
}

pub fn abort_transaction_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::Abandon,
    }
}

pub fn trap() -> ! {
    panic_any(Box::new(deterministic_error()))
}

thread_local! {
    static PANIC_HOOK_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

static INSTALL_HOOK: Once = Once::new();

fn install_panic_hook() {
    INSTALL_HOOK.call_once(|| {
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            let active = PANIC_HOOK_ACTIVE.with(|c| c.get());
            if !active {
                prev_hook(panic_info);
                return;
            }

            if let Some(payload) =
                panic_info.payload().downcast_ref::<InvokeError>()
            {
                panic::resume_unwind(Box::new(payload.clone()));
            }

            if let Some(payload) =
                panic_info.payload().downcast_ref::<String>()
            {
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
    });
}

pub fn activate_panic_hook() {
    install_panic_hook();
    PANIC_HOOK_ACTIVE.with(|c| c.set(true));
}

pub fn deactivate_panic_hook() {
    PANIC_HOOK_ACTIVE.with(|c| c.set(false));
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
                panic: InvokeErrorCode::None.0,
            },
            Err(err) => {
                if let Some(my_error) = err.downcast_ref::<InvokeError>() {
                    return groundhog_sys::ForeignInvokeResult {
                        panic: my_error.error_type.0,
                    };
                }
                eprintln!("unrecoverable error: {:?}", err);
                groundhog_sys::ForeignInvokeResult {
                    panic: InvokeErrorCode::Unrecoverable.0,
                }
            }
        }
    })
}
