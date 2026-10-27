use thruput_trace::ffi_enums::InvokeErrorCode;
use std::cell::Cell;
use std::error::Error;
use std::fmt;
use std::panic;
use std::panic::UnwindSafe;
use std::sync::Once;

/// Panic payload classifying how an invocation frame terminated; downcast
/// at the frame's catch site and mapped to the frame outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct InvokeError {
    /// The code the catch site reports to the executor.
    pub error_type: InvokeErrorCode,
}

impl fmt::Display for InvokeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InvokeError {}", self.error_type)
    }
}

impl Error for InvokeError {}

/// Payload for a deterministic contract failure.
pub fn deterministic_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::Deterministic,
    }
}

/// Payload for an out-of-gas termination.
pub fn out_of_gas_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::OutOfGas,
    }
}

/// Payload for an unrecoverable harness/FFI failure (not a contract fault).
pub fn unrecoverable_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::Unrecoverable,
    }
}

/// Panic payload for the non-error control-flow unwind (`raw_Return`
/// terminating an invocation frame). Despite traveling as an
/// [`InvokeError`], the code is [`InvokeErrorCode::None`]: the frame
/// completed; the panic only unwinds the contract's Rust stack. `raw_Exit`
/// is not one of these: it reverts the frame, so it unwinds as a
/// [`deterministic_error`].
pub fn control_flow_unwind() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::None,
    }
}

/// Payload for a validation failure.
pub fn validation_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::ValidationFailure,
    }
}

/// Payload for abandoning the whole transaction.
pub fn abort_transaction_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::Abandon,
    }
}

thread_local! {
    /// Nested contract-code regions open on this thread; see
    /// [`PanicHookGuard`], which is the only thing that raises it. hogd's
    /// `MockRuntime` captures its owning handler for the same thread reason
    /// instead of reading the thread-local context slot.
    static PANIC_HOOK_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Where a contract body panicked.
///
/// The SDK holds this same struct inside its `FaultInfo` rather than
/// mirroring it, so a location is described in one place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanicLocation {
    /// Source file, as `panic!` recorded it.
    pub file: String,
    /// 1-based line within [`file`](Self::file).
    pub line: u32,
    /// 1-based column within the line.
    pub column: u32,
}

impl std::fmt::Display for PanicLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

/// What the harness could see of a contract body's panic.
///
/// The message comes from the payload at the catch site and the location from
/// the panic hook, which is the only place it exists — `catch_unwind` hands
/// back a payload and nothing else. Either half can be absent: a `panic_any`
/// of a non-string type has no message, and a panic reaching a catch site
/// without a matching hook record has no location.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PanicRecord {
    /// The panic's message, when its payload carried one.
    pub message: Option<String>,
    /// Where the panic happened, when the hook recorded it.
    pub location: Option<PanicLocation>,
}

thread_local! {
    /// What the hook saw of the panic currently unwinding this thread.
    static HOOKED_PANIC: std::cell::RefCell<Option<PanicRecord>> =
        const { std::cell::RefCell::new(None) };
}

/// Take the record the hook stashed for the panic now being caught.
///
/// The hook runs during the unwind on the *same thread* that then catches it
/// — see [`PanicHookGuard`], whose docs spell out why a body and the guard
/// around it always share a thread. That makes this handoff thread-local by
/// construction, unlike anything between a caller and its callee, which run
/// on whichever pool threads resumed them.
///
/// `payload` is the authority on the message; a stashed record whose message
/// disagrees belongs to some other panic (one hooked but never caught here)
/// and contributes no location rather than a misleading one.
pub(crate) fn take_panic_record(
    payload: &(dyn std::any::Any + Send),
) -> PanicRecord {
    let message = panic_payload_message(payload);
    let location = take_hooked_panic()
        .filter(|hooked| hooked.message == message)
        .and_then(|hooked| hooked.location);
    PanicRecord { message, location }
}

/// Take whatever the hook stashed on this thread, leaving the slot empty.
fn take_hooked_panic() -> Option<PanicRecord> {
    HOOKED_PANIC.with(|slot| slot.borrow_mut().take())
}

/// Activates the classifying panic hook on the calling thread for as long as
/// it is held, installing it on first use.
///
/// Held around contract code and nothing else: by `trampoline` around the mock
/// body it invokes, and by `MockInvocationFrame::execute` around the lambda
/// standing in for a frame's body. A panic outside those is a harness bug, and
/// keeps the default output and an unrecoverable code -- which is why the real
/// FFI call in `TransactionHandler::root_invoke` holds no guard.
///
/// `trampoline` holding it is also what makes the mark land on the right
/// thread: execution is coroutinized, so a body runs on whichever pool thread
/// resumed it, but `trampoline` and the body it calls are always the same one.
///
/// A count, not a flag, since these nest: a frame's lambda invokes bodies, and
/// a body can invoke another contract. Decrements on drop, so an unwinding
/// body still leaves the count balanced.
pub struct PanicHookGuard;

impl PanicHookGuard {
    /// Enter a contract body on the calling thread.
    pub fn enter() -> Self {
        install_panic_hook();
        PANIC_HOOK_DEPTH.with(|c| c.set(c.get() + 1));
        PanicHookGuard
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        PANIC_HOOK_DEPTH.with(|c| c.set(c.get().saturating_sub(1)));
    }
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

            // While an invocation is active the hook only controls printing;
            // it must not panic or resume_unwind — since rust-lang/rust#158369
            // (nightly-2026-06-26, Rust 1.98) that aborts the process.
            // [`InvokeError`] payloads are expected control flow (aborts,
            // frame unwinds), so they stay silent; anything else is a
            // contract panic, printed compactly here in place of the default
            // hook's output. Either way the original payload keeps unwinding
            // to the enclosing catch site ([`invoke_wrapper`] or the mock
            // frame executor), which maps non-[`InvokeError`] payloads to a
            // deterministic error.
            if panic_info.payload().downcast_ref::<InvokeError>().is_none() {
                let record = PanicRecord {
                    message: panic_payload_message(panic_info.payload()),
                    location: panic_info.location().map(|l| PanicLocation {
                        file: l.file().to_string(),
                        line: l.line(),
                        column: l.column(),
                    }),
                };
                print_panic(&record, panic_info.payload());
                // Stash what only the hook can see. The catch site takes it
                // back and cross-checks the message, so a record left behind
                // by a panic caught somewhere else cannot be misread as this
                // one's.
                HOOKED_PANIC.with(|slot| *slot.borrow_mut() = Some(record));
            }
        }));
    });
}

/// Print a contract panic compactly, in place of the default hook's output.
///
/// Reads the already-captured `record` rather than re-inspecting the hook
/// info, so the text and the value the catch site receives cannot drift.
/// `payload` is only for the non-string case, which has no message.
fn print_panic(record: &PanicRecord, payload: &(dyn std::any::Any + Send)) {
    match &record.message {
        Some(message) => eprintln!("Panic message: {}", message),
        None => eprintln!("Panic: {:?}", payload),
    }

    if let Some(location) = &record.location {
        eprintln!(
            "Panic occurred at file '{}' line {}",
            location.file, location.line
        );
    }
}

/// Whether this thread is inside contract code, and so whether a panic on it
/// is contract code failing rather than a harness bug.
pub(crate) fn is_panic_hook_active() -> bool {
    PANIC_HOOK_DEPTH.with(|c| c.get()) > 0
}

/// The message from a panic payload, when it carries one.
///
/// Covers the two shapes `panic!` produces — a `&'static str` for a literal
/// message and a `String` for a formatted one. Any other payload (a
/// `panic_any` of some other type) has no message to report and yields
/// `None` rather than a placeholder, so a caller can tell "no message" from
/// a message that happens to read like one.
fn panic_payload_message(
    payload: &(dyn std::any::Any + Send),
) -> Option<String> {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        Some((*s).to_string())
    } else {
        payload.downcast_ref::<String>().cloned()
    }
}

/// Wrap a contract closure in the catch site that maps its panics to
/// Thruput invoke error codes.
///
/// `Sync` is the load-bearing bound: the wrapper is stored on the contract and
/// invoked through a shared reference from whichever thread resumed the
/// coroutine, so `&F` has to be usable there. Cloning an `Rc` capture from two
/// threads at once would race its refcount, and `Sync` rejects that at the
/// registration site instead. `Send` goes with it because the captures travel
/// with the contract's heap allocation.
pub fn invoke_wrapper<'a, F>(
    wrapped: F,
) -> Box<dyn Fn() -> thruput_sys::ForeignInvokeResult + Send + Sync + 'a>
where
    F: Fn() + UnwindSafe + Clone + Send + Sync + 'a,
{
    Box::new(move || {
        let w = wrapped.clone();
        let result = panic::catch_unwind(w);

        match result {
            Ok(_) => thruput_sys::ForeignInvokeResult {
                panic: InvokeErrorCode::None.0,
            },
            Err(err) => {
                if let Some(my_error) = err.downcast_ref::<InvokeError>() {
                    return thruput_sys::ForeignInvokeResult {
                        panic: my_error.error_type.0,
                    };
                }
                if is_panic_hook_active() {
                    // An ordinary panic out of contract code; the hook
                    // already printed it. Deterministic failure of this
                    // invocation.
                    //
                    // Drop the hook's record rather than leave it for some
                    // later frame to take: this catch site cannot attribute
                    // a message (it has no frame in hand), but it did catch
                    // the panic, so every catch site takes what it caught
                    // and a stale record cannot outlive its panic.
                    drop(take_hooked_panic());
                    return thruput_sys::ForeignInvokeResult {
                        panic: InvokeErrorCode::Deterministic.0,
                    };
                }
                eprintln!("unrecoverable error: {:?}", err);
                thruput_sys::ForeignInvokeResult {
                    panic: InvokeErrorCode::Unrecoverable.0,
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A contract panic must classify the same way wherever the body runs:
    // execution is coroutinized, so a body resumes on whichever pool thread
    // picked it up rather than the one that dispatched to it. `trampoline`
    // marks that thread, which is what this covers.
    #[test]
    fn mock_body_panic_is_deterministic_on_any_thread() {
        // Off the activating thread and outside a body, an ordinary panic is
        // a harness bug -- nothing says a frame is running.
        let unmarked = std::thread::spawn(|| {
            invoke_wrapper(|| panic!("contract panic"))().panic
        })
        .join()
        .expect("harness thread panicked");
        assert_eq!(unmarked, InvokeErrorCode::Unrecoverable.0);

        // Inside a body, on that same foreign thread, it is a deterministic
        // failure of the invocation -- which is the classification the
        // executor needs and a thread-local activation depth cannot give.
        let in_body = std::thread::spawn(|| {
            let _in_body = PanicHookGuard::enter();
            invoke_wrapper(|| panic!("contract panic"))().panic
        })
        .join()
        .expect("harness thread panicked");
        assert_eq!(in_body, InvokeErrorCode::Deterministic.0);
    }

    #[test]
    fn panic_hook_guard_balances_on_unwind() {
        assert!(!is_panic_hook_active());
        let caught = std::panic::catch_unwind(|| {
            let _in_body = PanicHookGuard::enter();
            assert!(is_panic_hook_active());
            panic!("body panicked");
        });
        assert!(caught.is_err());
        assert!(
            !is_panic_hook_active(),
            "guard must decrement when the body unwinds through it"
        );
    }

    #[test]
    fn display_terminates_and_names_the_error_code() {
        // Regression: Display used to format `self.clone()` with `{}`,
        // recursing infinitely and crashing on any to_string().
        let msg = deterministic_error().to_string();
        assert!(msg.contains("InvokeError"), "got: {msg}");
        assert!(msg.contains("Deterministic"), "got: {msg}");
    }

    #[test]
    fn hook_never_panics_and_leaves_payload_untouched() {
        // Regression for rust-lang/rust#158369 (first in nightly-2026-06-26):
        // the hook used to resume_unwind a converted payload from inside the
        // hook, which now aborts the whole process. Merely completing this
        // test proves the hook no longer panics while active.
        let _in_body = PanicHookGuard::enter();
        let err =
            panic::catch_unwind(|| panic!("contract panic")).unwrap_err();
        assert_eq!(
            err.downcast_ref::<&str>(),
            Some(&"contract panic"),
            "the hook must let the original payload unwind unchanged; \
             conversion is the catch site's job"
        );
    }

    #[test]
    fn invoke_wrapper_converts_panics_inside_a_body() {
        {
            // Inside a body: an ordinary contract panic becomes a
            // deterministic error at the catch site.
            let _in_body = PanicHookGuard::enter();
            let result = invoke_wrapper(|| panic!("contract panic"))();
            assert_eq!(result.panic, InvokeErrorCode::Deterministic.0);

            // InvokeError payloads pass through with their own code.
            let result =
                invoke_wrapper(|| std::panic::panic_any(validation_error()))();
            assert_eq!(result.panic, InvokeErrorCode::ValidationFailure.0);
        }

        // Outside one, an ordinary panic is a harness bug, not a contract
        // failure.
        assert!(!is_panic_hook_active());
        let result = invoke_wrapper(|| panic!("outside a body"))();
        assert_eq!(result.panic, InvokeErrorCode::Unrecoverable.0);
    }

    #[test]
    fn nested_body_stays_active_after_inner_guard_drops() {
        assert!(!is_panic_hook_active());

        let outer = PanicHookGuard::enter();
        {
            // A mock body invoking another contract re-enters `trampoline`.
            let _inner = PanicHookGuard::enter();
        }

        // The inner drop must not end the outer body: a panic here is still
        // a contract failure. This is why the marker counts rather than
        // flags.
        assert!(is_panic_hook_active());
        let result = invoke_wrapper(|| panic!("mid-nest panic"))();
        assert_eq!(result.panic, InvokeErrorCode::Deterministic.0);

        drop(outer);
        assert!(!is_panic_hook_active());
    }
}
