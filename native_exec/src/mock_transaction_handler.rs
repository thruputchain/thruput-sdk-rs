use crate::invoke_error::InvokeError;
use crate::test_environment::TestEnvironment;
use core::ffi::c_void;
use groundhog_sys::ForeignTryCommitResult;
use groundhog_trace::InvocationFrameTrace;
use groundhog_xdr::auth_claim::AuthClaimData;
use groundhog_xdr::transaction::InvocationFrameOutput;
use groundhog_xdr::types::SHA256Hash;
use std::cell::RefCell;
use std::panic::UnwindSafe;
use std::panic::panic_any;
use std::rc::Rc;

use groundhog_sys::FFIErrorCode;

#[derive(Debug, PartialEq)]
enum MockTransactionState {
    NotStarted,
    Running,
    RootInvokeFinished,
    Aborted,
    Committed,
}

#[derive(Debug, PartialEq)]
enum MockInvocationFrameState {
    Started,
    Closed(InvocationFrameOutput, Option<InvocationFrameTrace>),
}

pub struct MockInvocationFrameConfig<'a> {
    context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
    invocation: groundhog_xdr::transaction::Invocation,
    invoke_flags: i32,

    root_handler: *mut c_void,
    // when we build the frame, the new frame will be emplaced into this value
    to_emplace: &'a mut Option<Box<MockInvocationFrame>>,
}

impl<'a> MockInvocationFrameConfig<'a> {
    fn new(
        root_handler: *mut c_void,
        to_emplace: &'a mut Option<Box<MockInvocationFrame>>,
        context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
    ) -> Self {
        Self {
            context_pushcount,
            root_handler,
            invocation: groundhog_xdr::transaction::Invocation {
                invoked_address: [0; 32].into(),
                entrypoint: [0; 8].into(),
                gas_limit: 0,
                calldata: xdr_common::BytesM::default(),
            },
            invoke_flags: 0,
            to_emplace,
        }
    }

    pub fn at_contract_address(
        mut self,
        contract_address: groundhog_xdr::types::ContractAddress,
    ) -> Self {
        self.invocation.invoked_address = contract_address;
        self
    }

    pub fn with_calldata(mut self, calldata: xdr_common::BytesM) -> Self {
        self.invocation.calldata = calldata;
        self
    }

    pub fn with_invoke_flags(mut self, invoke_flags: i32) -> Self {
        self.invoke_flags = invoke_flags;
        self
    }
    pub fn build(self) -> &'a mut Box<MockInvocationFrame> {
        let serialized_invocation: Vec<u8> =
            xdrs::txs::encode(&self.invocation).unwrap();
        let frame = Box::new(MockInvocationFrame::new(
            self.root_handler,
            &serialized_invocation,
            self.invoke_flags,
            self.context_pushcount.clone(),
        ));
        *self.to_emplace = Some(frame);
        self.to_emplace.as_mut().unwrap()
    }
}

pub struct MockInvocationFrame {
    root_handler: *mut c_void,
    child_frame: Option<Box<MockInvocationFrame>>,
    state: MockInvocationFrameState,
    context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
}

impl MockInvocationFrame {
    fn new(
        root_handler: *mut c_void,
        serialized_invocation: &[u8],
        invoke_flags: i32,
        context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
    ) -> Self {
        let ffi_invocation = groundhog_sys::FFISerializedInvocation {
            data: serialized_invocation.as_ptr() as *mut u8,
            len: serialized_invocation.len().try_into().unwrap(),
        };

        let handler_res: groundhog_sys::FFIErrorCode = unsafe {
            groundhog_sys::open_mock_invocation_frame(
                root_handler,
                ffi_invocation,
                invoke_flags,
            )
            .into()
        };
        if handler_res != FFIErrorCode::None {
            eprintln!(
                "failed to open mock invocation frame: {:?}",
                handler_res
            );
            panic_any(crate::invoke_error::unrecoverable_error());
        }

        Self {
            root_handler,
            child_frame: None,
            state: MockInvocationFrameState::Started,
            context_pushcount,
        }
    }

    pub fn open_child_frame(&mut self) -> MockInvocationFrameConfig<'_> {
        let config = MockInvocationFrameConfig::new(
            self.root_handler,
            &mut self.child_frame,
            self.context_pushcount.clone(),
        );
        config
    }

    fn close_frame_internal(
        &mut self,
        error_code: hogexec_internal::ffi_enums::InvokeErrorCode,
    ) {
        let handler_res = unsafe {
            groundhog_sys::finish_mock_invocation_frame(
                self.root_handler,
                error_code.0,
            )
        };
        let err: FFIErrorCode = handler_res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            eprintln!("failed to close mock invocation frame: {:?}", err);
            panic_any(crate::invoke_error::unrecoverable_error());
        }

        let output: InvocationFrameOutput = xdrs::txs::decode(
            handler_res.serialized_output.as_slice().unwrap(),
        )
        .unwrap();
        let trace: Option<InvocationFrameTrace> = if let Some(trace_data) =
            handler_res.serialized_trace.as_slice()
        {
            let deserialized_trace: InvocationFrameTrace =
                xdrs::txs::decode(trace_data).unwrap();
            Some(deserialized_trace)
        } else {
            None
        };
        self.state = MockInvocationFrameState::Closed(output, trace);
    }

    fn push_context(&mut self) {
        std::println!("top level push context");
        self.context_pushcount
            .borrow_mut()
            .push_context(self.root_handler);
    }

    fn pop_context(&mut self) {
        self.context_pushcount.borrow_mut().pop_context();
    }

    // return true if the frame could be continued
    pub fn execute(
        &mut self,
        lambda: impl FnOnce(&mut Self) -> bool + UnwindSafe,
    ) -> &mut Self {
        if self.state != MockInvocationFrameState::Started {
            eprintln!(
                "Should be impossible to execute a frame while it is not started"
            );
            panic_any(crate::invoke_error::unrecoverable_error());
        }

        self.push_context();

        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                lambda(self)
            }));

        self.pop_context();

        match result {
            Ok(result) => {
                if !result {
                    self.close_frame_internal(hogexec_internal::ffi_enums::InvokeErrorCode::kInvokeNone);
                }
            }
            Err(err) => {
                if let Some(my_error) = err.downcast_ref::<InvokeError>() {
                    self.close_frame_internal(my_error.error_type);
                } else {
                    self.close_frame_internal(hogexec_internal::ffi_enums::InvokeErrorCode::kInvokeUnrecoverable);
                }
            }
        };
        self
    }

    pub fn result(
        &mut self,
    ) -> (InvocationFrameOutput, Option<InvocationFrameTrace>) {
        let res = match &self.state {
            MockInvocationFrameState::Closed(output, trace) => {
                (output.clone(), trace.clone())
            }
            _ => {
                eprintln!(
                    "Cannot get the result of a frame while it is not closed"
                );
                panic_any(crate::invoke_error::unrecoverable_error());
            }
        };
        self.child_frame = None;
        res
    }
}

impl Drop for MockInvocationFrame {
    fn drop(&mut self) {
        if self.state == MockInvocationFrameState::Started {
            panic!(
                "Should be impossible to drop an invocation frame while it is running"
            );
        }
    }
}

struct ThreadlocalContextPushcount {
    count: i32,
}

impl ThreadlocalContextPushcount {
    pub fn new() -> Self {
        Self { count: 0 }
    }

    pub fn push_context(&mut self, handler: *mut c_void) {
        std::println!("pushing context, count: {}", self.count);
        if self.count == 0 {
            let push_res = unsafe { groundhog_sys::push_context(handler) };
            let err: FFIErrorCode = push_res.into();
            if err != FFIErrorCode::None {
                panic!("failed to push context: {:?}", err);
            }
        }
        self.count += 1;
    }

    pub fn pop_context(&mut self) {
        if self.count == 0 {
            panic!("cannot pop context if count is 0");
        }
        self.count -= 1;
        if self.count == 0 {
            let pop_res = unsafe { groundhog_sys::pop_context() };
            let err: FFIErrorCode = pop_res.into();
            if err != FFIErrorCode::None {
                panic!("failed to pop context: {:?}", err);
            }
        }
    }
}

impl Drop for ThreadlocalContextPushcount {
    fn drop(&mut self) {
        if self.count != 0 {
            eprintln!(
                "Dropping transaction while threadlocal context active: count={}",
                self.count
            );
            let pop_res = unsafe { groundhog_sys::pop_context() };
            let err: FFIErrorCode = pop_res.into();
            if err != FFIErrorCode::None {
                panic!("failed to pop context: {:?}", err);
            }
        }
    }
}

// Can run one, and only one, transaction,
// and then get the results
pub struct MockTransactionHandler {
    handler: *mut c_void,
    _env: Rc<TestEnvironment>,
    state: MockTransactionState,
    root_frame: Option<Box<MockInvocationFrame>>,
    context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
}

impl MockTransactionHandler {
    pub fn new(env: Rc<TestEnvironment>) -> Self {
        // TODO(geoff): ways to mock this out
        let auth_claim_data: AuthClaimData = AuthClaimData::default();
        let serialized_auth_claim_data =
            xdrs::txs::encode(&auth_claim_data).unwrap();
        let source_tx_hash: SHA256Hash = [0; 32].into();
        let handler_res = unsafe {
            groundhog_sys::open_mocked_syscall_handler(
                env.inner.borrow().db.db,
                env.inner.borrow().contract_context.context,
                groundhog_sys::FFISerializedAuthClaimData {
                    data: serialized_auth_claim_data.as_ptr() as *mut u8,
                    len: serialized_auth_claim_data.len().try_into().unwrap(),
                },
                source_tx_hash.as_ptr(),
            )
        };
        let err: FFIErrorCode = handler_res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to open mocked syscall handler: {:?}", err);
        }
        assert!(!handler_res.syscall_handler.is_null());
        Self {
            handler: handler_res.syscall_handler,
            _env: env,
            state: MockTransactionState::NotStarted,
            root_frame: None,
            context_pushcount: Rc::new(RefCell::new(
                ThreadlocalContextPushcount::new(),
            )),
        }
    }

    pub fn abort_transaction(&mut self) {
        assert_eq!(self.state, MockTransactionState::RootInvokeFinished);
        self.state = MockTransactionState::Aborted;
        let err: FFIErrorCode =
            unsafe { groundhog_sys::abort_transaction(self.handler) }.into();
        if err != FFIErrorCode::None {
            panic!("failed to abort transaction: {:?}", err);
        }
    }

    pub fn try_commit_transaction(
        &mut self,
    ) -> groundhog_xdr::exec_result::ExecResult {
        assert_eq!(self.state, MockTransactionState::RootInvokeFinished);
        self.state = MockTransactionState::Committed;
        let result: ForeignTryCommitResult =
            unsafe { groundhog_sys::try_commit_transaction(self.handler) };
        let err: FFIErrorCode = result.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("ffi error: failed to commit transaction: {:?}", err);
        }
        self.state = MockTransactionState::Committed;

        let exec_result: groundhog_xdr::exec_result::ExecResult =
            result.result_code.try_into().unwrap();
        exec_result
    }

    pub fn open_root_frame(&mut self) -> MockInvocationFrameConfig<'_> {
        let config = MockInvocationFrameConfig::new(
            self.handler,
            &mut self.root_frame,
            self.context_pushcount.clone(),
        );
        config
    }
}

impl Drop for MockTransactionHandler {
    fn drop(&mut self) {
        if self.state == MockTransactionState::Running {
            panic!(
                "Should be impossible to drop a transaction handler while it is running"
            );
        }
        if self.state == MockTransactionState::RootInvokeFinished {
            eprintln!(
                "Cleanup: Aborting mock transaction that finished root_invoke but was not committed"
            );
            self.abort_transaction();
        }

        let err: FFIErrorCode =
            unsafe { groundhog_sys::close_syscall_handler(self.handler) }
                .into();
        if err != FFIErrorCode::None {
            panic!("failed to close syscall handler: {:?}", err);
        }
    }
}

#[cfg(test)]
mod tests {
    use groundhog_xdr::transaction::InvocationFrameStatusCode;

    use super::*;

    #[test]
    fn test_mock_transaction_handler() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        static mut FIRST: bool = false;
        static mut SECOND: bool = false;

        let (output, _) = handler
            .open_root_frame()
            .with_invoke_flags(0)
            .build()
            .execute(|_| {
                unsafe {
                    FIRST = true;
                }
                true
            })
            .execute(|_| {
                unsafe {
                    SECOND = true;
                }
                false
            })
            .result();

        assert!(unsafe { FIRST });
        assert!(unsafe { SECOND });
        assert_eq!(
            output.status_code(),
            InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn.0
        );
    }

    #[test]
    fn test_mock_transaction_handler_with_error() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        let (output, _) = handler
            .open_root_frame()
            .with_invoke_flags(0)
            .build()
            .execute(|_| {
                panic!("test error");
            })
            .result();
        assert_eq!(
            output.status_code(),
            InvocationFrameStatusCode::kInvocationFrameStatusCodeFault.0
        );
    }

    #[test]
    fn test_nested_mocks() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        static mut FIRST: bool = false;
        static mut SECOND: bool = false;

        let (output, _) = handler
            .open_root_frame()
            .with_invoke_flags(0)
            .build()
            .execute(|frame| {
                frame
                    .open_child_frame()
                    .with_invoke_flags(0)
                    .build()
                    .execute(|_| {
                        unsafe {
                            assert!(!FIRST);
                            SECOND = true;
                        }
                        false
                    })
                    .result();

                unsafe {
                    assert!(SECOND);
                    FIRST = true;
                }
                false
            })
            .result();
        assert_eq!(
            output.status_code(),
            InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn.0
        );
        assert!(unsafe { FIRST });
        assert!(unsafe { SECOND });
    }

    #[test]
    fn test_syscalls_with_context() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        let (output, _) = handler
            .open_root_frame()
            .with_invoke_flags(0)
            .at_contract_address(groundhog_xdr::types::ContractAddress::from(
                [1; 32],
            ))
            .build()
            .execute(|_| {
                std::println!("getting current address");
                let res = groundhog_xdr::syscalls::GetCurrentAddress();
                std::println!("res: {:?}", res);
                assert_eq!(
                    res.unwrap(),
                    groundhog_xdr::types::ContractAddress::from([1; 32])
                );
                false
            })
            .result();
        assert_eq!(
            output.status_code(),
            InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn.0
        );
    }

    #[test]
    fn test_concurrent_transactions() {
        let env = TestEnvironment::new(None);
        let mut handler1 = MockTransactionHandler::new(env.clone());
        let mut handler2 = MockTransactionHandler::new(env);

        let frame1 = handler1
            .open_root_frame()
            .with_invoke_flags(0)
            .at_contract_address(groundhog_xdr::types::ContractAddress::from(
                [1; 32],
            ))
            .build()
            .execute(|_| {
                std::println!("getting current address");
                let res = groundhog_xdr::syscalls::GetCurrentAddress();
                std::println!("res: {:?}", res);
                assert_eq!(
                    res.unwrap(),
                    groundhog_xdr::types::ContractAddress::from([1; 32])
                );
                true
            });

        let frame2 = handler2
            .open_root_frame()
            .with_invoke_flags(0)
            .at_contract_address(groundhog_xdr::types::ContractAddress::from(
                [2; 32],
            ))
            .build()
            .execute(|_| {
                std::println!("getting current address");
                let res = groundhog_xdr::syscalls::GetCurrentAddress();
                std::println!("res: {:?}", res);
                assert_eq!(
                    res.unwrap(),
                    groundhog_xdr::types::ContractAddress::from([2; 32])
                );
                true
            });
        let (output1, _) = frame1
            .execute(|_| {
                std::println!("getting current address");
                let res = groundhog_xdr::syscalls::GetCurrentAddress();
                std::println!("res: {:?}", res);
                assert_eq!(
                    res.unwrap(),
                    groundhog_xdr::types::ContractAddress::from([1; 32])
                );
                false
            })
            .result();

        let (output2, _) = frame2
            .execute(|_| {
                std::println!("getting current address");
                let res = groundhog_xdr::syscalls::GetCurrentAddress();
                std::println!("res: {:?}", res);
                assert_eq!(
                    res.unwrap(),
                    groundhog_xdr::types::ContractAddress::from([2; 32])
                );
                false
            })
            .result();

        assert_eq!(
            output1.status_code(),
            InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn.0
        );
        assert_eq!(
            output2.status_code(),
            InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn.0
        );
    }
}
