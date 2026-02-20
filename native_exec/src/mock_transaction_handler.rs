use crate::test_environment::TestEnvironment;
use core::ffi::c_void;
use groundhog_trace::InvocationFrameTrace;
use groundhog_xdr::auth_claim::AuthClaimData;
use groundhog_xdr::transaction::InvocationFrameOutput;
use groundhog_xdr::types::SHA256Hash;
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
    Closed,
}

pub struct MockInvocationFrameConfig<'a> {
    invocation: groundhog_xdr::transaction::Invocation,
    invoke_flags: i32,

    root_handler: *mut c_void,
    // when we build the frame, the new frame will be emplaced into this value
    to_emplace: &'a mut Option<Box<MockInvocationFrame>>,
}

impl<'a> MockInvocationFrameConfig<'a> {
    pub fn new(
        root_handler: *mut c_void,
        to_emplace: &'a mut Option<Box<MockInvocationFrame>>,
    ) -> Self {
        Self {
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
    pub fn build(self) -> () {
        let serialized_invocation: Vec<u8> =
            xdrs::txs::encode(&self.invocation).unwrap();
        let frame = Box::new(MockInvocationFrame::new(
            self.root_handler,
            &serialized_invocation,
            self.invoke_flags,
        ));
        *self.to_emplace = Some(frame);
    }
}

pub struct MockInvocationFrame {
    root_handler: *mut c_void,
    child_frame: Option<Box<MockInvocationFrame>>,
    state: MockInvocationFrameState,
}

impl MockInvocationFrame {
    pub fn new(
        root_handler: *mut c_void,
        serialized_invocation: &[u8],
        invoke_flags: i32,
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
            panic!("failed to open mock invocation frame: {:?}", handler_res);
        }

        Self {
            root_handler,
            child_frame: None,
            state: MockInvocationFrameState::Started,
        }
    }

    pub fn open_child_frame(&mut self) -> MockInvocationFrameConfig<'_> {
        let config = MockInvocationFrameConfig::new(
            self.root_handler,
            &mut self.child_frame,
        );
        config
    }

    fn close_frame_internal(
        &mut self,
        error_code: hogexec_internal::ffi_enums::InvokeErrorCode,
    ) -> (InvocationFrameOutput, Option<InvocationFrameTrace>) {
        let handler_res = unsafe {
            groundhog_sys::finish_mock_invocation_frame(
                self.root_handler,
                error_code as i32,
            )
        };
        let err: FFIErrorCode = handler_res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to close mock invocation frame: {:?}", err);
        }
        self.state = MockInvocationFrameState::Closed;

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
        (output, trace)
    }

    pub fn close_frame_commit(
        &mut self,
    ) -> (InvocationFrameOutput, Option<InvocationFrameTrace>) {
        self.close_frame_internal(
            hogexec_internal::ffi_enums::InvokeErrorCode::kInvokeNone,
        )
    }

    pub fn close_frame_fault(
        &mut self,
    ) -> (InvocationFrameOutput, Option<InvocationFrameTrace>) {
        self.close_frame_internal(
            hogexec_internal::ffi_enums::InvokeErrorCode::kInvokeDeterministic,
        )
    }
}

impl Drop for MockInvocationFrame {
    fn drop(&mut self) {
        if self.state == MockInvocationFrameState::Closed {
            panic!(
                "Should be impossible to drop an invocation frame while it is running"
            );
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

    fn assert_running(&self) {
        assert_eq!(self.state, MockTransactionState::Running);
    }
    fn assert_not_finished(&self) {
        assert_ne!(self.state, MockTransactionState::RootInvokeFinished);
    }

    pub fn get_root_frame(&self) -> &Option<Box<MockInvocationFrame>> {
        // Lifetime is automatically tied to &self (the handler)
        &self.root_frame
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
