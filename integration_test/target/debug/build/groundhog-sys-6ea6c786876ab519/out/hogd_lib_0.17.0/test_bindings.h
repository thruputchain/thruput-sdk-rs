#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

// Version constants for test_ffi library
// These are checked against release tags in CI
#define TEST_FFI_VERSION_MAJOR 0
#define TEST_FFI_VERSION_MINOR 17
#define TEST_FFI_VERSION_PATCH 0

enum TestFFIErrorCode {
  kTestFFIErrorCodeNone = 0,
  // Nullptr passed as an argument
  kTestFFIErrorCodeInvalidPointer = 1,
  // Cannot deploy a contract that has not yet been created.
  kTestFFIErrorCodeDeployNonexistentContract = 2,
  // Cannot create a mock contract for a function pointer that has not yet been
  // registered with a mock ID.
  kTestFFIErrorCodeCreateNonexistentMockContract = 3,
  // Supplied data structs failed to deserialize into expected format
  kTestFFIErrorCodeDeserializeError = 4,
  // Unrecoverable error of unknown type
  kTestFFIErrorCodeUnrecoverableError = 5,
  // Contract failed wasm verification (e.g., memory exceeds limits)
  kTestFFIErrorCodeContractVerificationFailed = 6,
};

enum TestFFILogLevel {
  kTestFFILogLevelCritical = 0,
  kTestFFILogLevelError = 1,
  kTestFFILogLevelWarn = 2,
  kTestFFILogLevelInfo = 3,
  kTestFFILogLevelDebug = 4,
  kTestFFILogLevelTrace = 5,
};

// A buffer of data that can be passed across the FFI boundary.
// Must be freed with free_ffi_data_buffer()
struct FFIDataBuffer {
  uint8_t *data;
  uint64_t len;
};

enum TestFFIErrorCode free_ffi_data_buffer(struct FFIDataBuffer buffer);

// Translation of InvokeStatus<void> into an FFI-safe type.
// This is the type returned by an invocation to a mocked contract.
struct ForeignInvokeResult {
  int32_t panic; // if panic is InvokeErrorCode::None, this is translated
                 // to an InvokeStatus<void>{}
};

// FFI-safe type corresponding to the result of an invocation.
// InvocationFrameOutput, gas consumed, trace(optional), and events emitted
struct ForeignTracedInvokeResult {
  // Serialization of InvocationFrameOutpput
  struct FFIDataBuffer serialized_output;

  uint64_t gas_consumed;

  struct FFIDataBuffer serialized_trace;

  // Serialized events emitted during this frame (Vec<TxEmittedEvent>)
  struct FFIDataBuffer serialized_events;

  // Errors that arise from the FFI bindings, and NOT from the regular operation
  // of hogexec.
  enum TestFFIErrorCode ffi_error_code;
};

// Type of the function pointer used within a mock contract.
// NOLINTNEXTLINE(modernize-use-using)
typedef struct ForeignInvokeResult (*ForeignEntryPoint)(void *foreign_context,
                                                        uint64_t method_number);

// Translation of HostFnStatus<SyscallErrorCode> into an FFI-safe type.
// TODO(geoff): This is identical to TrampolineResult in runtime_trampoline.
// This could be replaced by the same type as there, although that could
// somewhat complicate the build system/bindgen usage.
struct ForeignHostFnResult {
  int32_t result_code;
  int32_t panic; // HostFnErrorCode
};

// Translation of try_commit_transaction result into an FFI-safe type.
struct ForeignTryCommitResult {
  int32_t result_code; // ExecResult

  // Errors that arise from the FFI bindings, and NOT from the regular operation
  // of hogexec.
  enum TestFFIErrorCode ffi_error_code;
};

struct CreateDBResult {
  void *db;
  enum TestFFIErrorCode ffi_error_code;
};

struct GetCostModelResult {
  struct FFIDataBuffer data;
  enum TestFFIErrorCode ffi_error_code;
};

struct FFISerializedAddressAndKey {
  const uint8_t *data;
  uint32_t len;
};

struct GetSnapshotStorageItemResult {
  // Serialized WireStorageItem. Empty if the key does not exist.
  struct FFIDataBuffer data;
  enum TestFFIErrorCode ffi_error_code;
};

struct CreateDBResult create_db(const uint8_t *serialized_cost_model,
                                uint32_t serialized_cost_model_len);
enum TestFFIErrorCode free_db(void *ptr);

struct GetCostModelResult get_cost_model(void *db);
struct GetSnapshotStorageItemResult
get_snapshot_storage_item(void *db, struct FFISerializedAddressAndKey addrkey);

enum TestFFIErrorCode advance_new_block(void *db);

struct OpenRuntimeContextResult {
  void *context;
  enum TestFFIErrorCode ffi_error_code;
};

struct OpenRuntimeContextResult open_runtime_context();
enum TestFFIErrorCode close_runtime_context(void *context);

struct FFISerializedSignedTransaction {
  uint8_t *data;
  uint32_t len;
};

// Returns 0 for success, nonzero for some error code
// These errors should not be handled by a contract,
// but are SDK or usage errors (i.e. invalid context pointer)
//
// Note that this does NOT cause the contract to be "created" within the
// database. That still requires some syscall (or setup) that creates a
// Contract(ContractType::kMockTest, id) within the database.  If that creation
// is performed without calling this method, instantiating the contract will
// fail.
enum TestFFIErrorCode register_new_contract(void *context,
                                            int32_t mock_contract_id,
                                            ForeignEntryPoint entry_point,
                                            void *foreign_context);

// Methods to assist in setup for a test.
// Note that methods for implementing view isolation of one block from a
// sibling block are not implemented; thus, these setup methods just write to
// whatever state is in the database.
// TODO(geoff): Update these methods, and the appropriate parts of the test sdk,
// when that code is merged into the db.
//
// setup_create_contract creates a contract that is _visible_ in the current
// block.
enum TestFFIErrorCode setup_create_contract(void *context, void *db,
                                            uint8_t const *contract_bytes,
                                            uint32_t contract_len);

// setup_create_contract creates a contract that is _visible_ in the current
// block.
// contract_hash must be a pointer to a 32-byte SHA256Hash.
// contract_size is the size of the contract in bytes.
enum TestFFIErrorCode setup_deploy_contract(void *db,
                                            const uint8_t *deployment_addr,
                                            const uint8_t *contract_hash,
                                            uint32_t contract_size);

enum TestFFIErrorCode
setup_grant_contract_storage_tokens(void *db, const uint8_t *contract_addr,
                                    int64_t tokens);

struct OpenSyscallHandlerResult {
  void *syscall_handler;
  enum TestFFIErrorCode ffi_error_code;
};
// db must have been returned by create_db()
// rc must have been returned by open_runtime_context()
struct OpenSyscallHandlerResult
open_syscall_handler(void *db, void *rc,
                     struct FFISerializedSignedTransaction signed_tx);

struct FFISerializedAuthClaimData {
  const uint8_t *data;
  uint32_t len;
};

struct FFISerializedInvocation {
  const uint8_t *data;
  uint32_t len;
};

struct OpenSyscallHandlerResult open_mocked_syscall_handler(
    void *db, void *rc, struct FFISerializedAuthClaimData auth_claim_data,
    const uint8_t source_tx_hash[32], uint8_t enable_tracing);

enum TestFFIErrorCode
open_mock_invocation_frame(void *syscall_handler,
                           struct FFISerializedInvocation invocation,
                           int32_t invoke_flags);

struct ForeignTracedInvokeResult
finish_mock_invocation_frame(void *syscall_handler, int32_t invoke_error_code);

// These need to be called before and after invoking system calls in a mock
// invocation frame.
enum TestFFIErrorCode push_context(void *syscall_handler);
enum TestFFIErrorCode pop_context();

enum TestFFIErrorCode close_syscall_handler(void *syscall_handler);

struct ForeignHostFnResult mock_syscall_entrypoint(int32_t syscall,
                                                   uint64_t arg1, uint64_t arg2,
                                                   uint64_t arg3, uint64_t arg4,
                                                   uint64_t arg5, uint64_t arg6,
                                                   uint64_t arg7);

struct ForeignTracedInvokeResult root_invoke(void *syscall_handler,
                                             uint8_t enable_tracing);
struct ForeignTryCommitResult try_commit_transaction(void *syscall_handler);
enum TestFFIErrorCode abort_transaction(void *syscall_handler);

void enable_syscall_logging(int32_t level);

struct FFISourceAuthData {
  const uint8_t *data;
  uint32_t len;
};

// Result of authorization verification
struct VerifyAuthResult {
  // 0 if authorization is valid, otherwise AuthVerifyErrorCode as i32
  int32_t auth_error_code;
  // FFI-level error code
  enum TestFFIErrorCode ffi_error_code;
};

// Verify authorization on a SignedTransaction
//
// signed_tx: serialized SignedTransaction
// source_auth_data: serialized AddressAuthData for the source address
struct VerifyAuthResult
verify_signed_transaction_auth(struct FFISerializedSignedTransaction signed_tx,
                               struct FFISourceAuthData source_auth_data);

// Note that AuthErrorCode is distinct from AuthVerifyErrorCode.
// AuthErrorCode includes additional checks, such as missing auth data and block
// height ranges.
struct VerifyAuthResultFromDBResult {
  int32_t auth_error_code; // AuthErrorCode
  enum TestFFIErrorCode ffi_error_code;
};

// Verify authorization on a SignedTransaction using auth data from database
//
// db: database context to read auth data from
// signed_tx: serialized SignedTransaction
struct VerifyAuthResultFromDBResult verify_signed_transaction_auth_from_db(
    void *db, struct FFISerializedSignedTransaction signed_tx);

// Result of contract wasm verification.
struct VerifyContractResult {
  // 1 if contract is valid, 0 if invalid
  uint8_t success;
  // FFI-level error code
  enum TestFFIErrorCode ffi_error_code;
};

// Verify a serialized Contract against the current runtime context.
// context: returned by open_runtime_context().
// contract_bytes/contract_len: XDR-serialized Contract (same format as
//                              setup_create_contract).
struct VerifyContractResult do_verify_contract(void *context,
                                               const uint8_t *contract_bytes,
                                               uint32_t contract_len);

#ifdef __cplusplus
}
#endif
