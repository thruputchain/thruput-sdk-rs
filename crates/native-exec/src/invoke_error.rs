use hogexec_internal::ffi_enums::InvokeErrorCode;
use std::cell::Cell;
use std::error::Error;
use std::fmt;
use std::panic;
use std::panic::UnwindSafe;
use std::sync::Once;

#[derive(Debug, Clone, PartialEq)]
pub struct InvokeError {
    pub error_type: InvokeErrorCode,
}

impl fmt::Display for InvokeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InvokeError {}", self.error_type)
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

thread_local! {
    static PANIC_HOOK_DEPTH: Cell<usize> = const { Cell::new(0) };
}

static INSTALL_HOOK: Once = Once::new();

fn install_panic_hook() {
    INSTALL_HOOK.call_once(|| {
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            if !is_panic_hook_active() {
                prev_hook(panic_info);
                return;
            }

            if let Some(payload) =
                panic_info.payload().downcast_ref::<InvokeError>()
            {
                panic::resume_unwind(Box::new(payload.clone()));
            }

            _print_panic_info(panic_info);

            panic::resume_unwind(Box::new(deterministic_error()));
        }));
    });
}

fn _print_panic_info(panic_info: &std::panic::PanicHookInfo<'_>) {
    if let Some(payload) = panic_info.payload().downcast_ref::<String>() {
        eprintln!("Panic message: {}", payload);
    } else if let Some(payload) = panic_info.payload().downcast_ref::<&str>() {
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
}

pub(crate) fn is_panic_hook_active() -> bool {
    panic_hook_depth() > 0
}

pub(crate) fn panic_hook_depth() -> usize {
    PANIC_HOOK_DEPTH.with(|c| c.get())
}

pub fn activate_panic_hook() {
    install_panic_hook();
    PANIC_HOOK_DEPTH.with(|c| c.set(c.get() + 1));
}

pub fn deactivate_panic_hook() {
    PANIC_HOOK_DEPTH.with(|c| {
        let depth = c.get();
        assert!(
            depth > 0,
            "deactivate_panic_hook called without matching activate"
        );
        c.set(depth.saturating_sub(1));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_terminates_and_names_the_error_code() {
        // Regression: Display used to format `self.clone()` with `{}`,
        // recursing infinitely and crashing on any to_string().
        let msg = deterministic_error().to_string();
        assert!(msg.contains("InvokeError"), "got: {msg}");
        assert!(msg.contains("Deterministic"), "got: {msg}");
    }

    #[test]
    fn panic_hook_converts_panics_when_active() {
        // Install the hook so it's ready
        install_panic_hook();

        // Without activation: a string panic stays a string panic
        assert_eq!(panic_hook_depth(), 0);
        let err = panic::catch_unwind(|| panic!("bare panic")).unwrap_err();
        assert!(
            err.downcast_ref::<InvokeError>().is_none(),
            "without activation, panic should NOT be converted to InvokeError"
        );

        // With activation: a string panic is converted to a deterministic InvokeError
        activate_panic_hook();
        assert_eq!(panic_hook_depth(), 1);

        let err =
            panic::catch_unwind(|| panic!("contract panic")).unwrap_err();
        let invoke_err = err.downcast_ref::<InvokeError>().expect(
            "with activation, panic should be converted to InvokeError",
        );
        assert_eq!(invoke_err.error_type, InvokeErrorCode::Deterministic);

        deactivate_panic_hook();
        assert_eq!(panic_hook_depth(), 0);

        // After deactivation: back to normal
        let err =
            panic::catch_unwind(|| panic!("after deactivate")).unwrap_err();
        assert!(
            err.downcast_ref::<InvokeError>().is_none(),
            "after deactivation, panic should NOT be converted to InvokeError"
        );
    }

    #[test]
    fn nested_activation_stays_active_after_inner_deactivate() {
        install_panic_hook();
        assert_eq!(panic_hook_depth(), 0);

        // Outer activate
        activate_panic_hook();
        assert_eq!(panic_hook_depth(), 1);

        // Inner activate (simulating nested contract call)
        activate_panic_hook();
        assert_eq!(panic_hook_depth(), 2);

        // Inner deactivate — hook must still be active
        deactivate_panic_hook();
        assert_eq!(panic_hook_depth(), 1);
        assert!(is_panic_hook_active());

        // Panic here should still be converted (this is the bug scenario:
        // before the fix, the inner deactivate would have turned off the hook)
        let err =
            panic::catch_unwind(|| panic!("mid-nest panic")).unwrap_err();
        let invoke_err = err
            .downcast_ref::<InvokeError>()
            .expect("hook should still be active after inner deactivate");
        assert_eq!(invoke_err.error_type, InvokeErrorCode::Deterministic);

        // Outer deactivate
        deactivate_panic_hook();
        assert_eq!(panic_hook_depth(), 0);
        assert!(!is_panic_hook_active());
    }

    #[test]
    #[should_panic(
        expected = "deactivate_panic_hook called without matching activate"
    )]
    fn deactivate_without_activate_panics() {
        assert_eq!(panic_hook_depth(), 0);
        deactivate_panic_hook();
    }
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
