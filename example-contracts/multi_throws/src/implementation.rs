use super::*;
use sdk::*;

#[derive(Default)]
pub struct Contract;

/// An error type NOT declared in throws — will end up in Unknown
#[contract_exception]
pub struct SecretError;

#[derive(WriteXDR, Debug, PartialEq, Eq)]
#[xdrs(crate = "sdk::__xdrs")]
pub struct UndecodableError {
    pub selector: u64,
    pub extra_payload: u32,
}

#[contract]
impl MultiThrows for Contract {
    fn succeed() -> u32 {
        42
    }

    fn throw_alpha_not_found() {
        throw(AlphaError::NotFound);
    }

    fn throw_alpha_overflow() {
        throw(AlphaError::Overflow);
    }

    fn throw_beta_denied() {
        throw(BetaError::Denied);
    }

    fn throw_undeclared() {
        throw(SecretError);
    }

    fn forward_alpha(target: ContractAddress) {
        // Non-try call: if the target throws, re_throw forwards the error
        MultiClient::new(&target).throw_alpha_not_found();
    }

    fn forward_undeclared(target: ContractAddress) {
        // Non-try call: SecretError isn't in throws, forwarded as raw bytes
        MultiClient::new(&target).throw_undeclared();
    }

    fn panics() {
        panic!("intentional panic");
    }

    fn forward_fault(target: ContractAddress) {
        // Non-try call: the inner panic should propagate as a fault
        MultiClient::new(&target).panics();
    }

    fn throw_undecodable() {
        contract_abort_encoded(&UndecodableError {
            selector: AlphaError::SELECTOR,
            extra_payload: 42,
        });
    }

    fn throw_malicious_library_error() {
        contract_abort_encoded(&UndecodableError {
            selector: LibraryExn::SELECTOR,
            extra_payload: 42,
        });
    }

    fn relay_undecodable(target: ContractAddress) {
        let client = MultiClient::new(&target);
        match client.try_throw_undecodable() {
            Err(MultiClientError::BadDecode(caught)) => {
                // Unrelated SDK op between catch and re-throw: `Hash::hash`
                // (crates/sdk/src/hash.rs) encodes into and reads back the
                // shared call scratch via `with_scratch`, so this genuinely
                // reuses the buffer that produced `caught`. Natively this
                // exercises the alloc/Vec `CaughtException`; the no-alloc
                // `EXN_SLOT` path is compiled (not executed) by the CI guest
                // builds.
                let _ = 42u32.hash();
                log(b"caught, doing other work before re-throw");
                caught.re_throw()
            }
            other => client_library_throw(LibraryExn::Called(
                FaultString::from_display(format_args!(
                    "expected BadDecode, got {other:?}"
                )),
            )),
        }
    }
}
