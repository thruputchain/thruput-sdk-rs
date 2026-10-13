//! Two-phase-commit conflict diagnostics surfaced through
//! `try_commit_transaction` as `TryCommitResult::TxNonDeterministicError`.
//!
//! Two transactions in the same block that write different bytestrings to the
//! same key(s) collide at commit; the loser's commit fails nondeterministically
//! and carries a decoded [`DbConflictResult`] listing the conflicting slots.

use thruput_sdk::sys_exn::bytestring::*;
use thruput_sdk::*;

const ADDR: ContractAddress = ContractAddress::new([1u8; 32]);
const KEY_A: StorageKey = StorageKey::new([0xAA; 32]);
const KEY_B: StorageKey = StorageKey::new([0xBB; 32]);

/// Extract the conflict list from a nondeterministic commit result, asserting
/// the database reported the full (`kDbDiagnosticAll`) diagnostic.
#[track_caller]
fn conflicts_of(result: TryCommitResult) -> Vec<DbConflictDiagnostic> {
    match result {
        TryCommitResult::TxNonDeterministicError(conflict) => match conflict {
            DbConflictResult::kDbDiagnosticAll { conflicts } => {
                conflicts.into()
            }
            other => {
                panic!("expected kDbDiagnosticAll diagnostics, got {other:?}")
            }
        },
        other => panic!("expected a nondeterministic conflict, got {other:?}"),
    }
}

#[test]
fn bytestring_single_conflict_decodes() {
    let env = TestEnvironment::no_cost();

    // First writer commits cleanly.
    env.transact(|tx| tx.call(&ADDR).run(|| bytestring_set(&KEY_A, b"first")))
        .assert_success();

    // Second writer in the same block writes a different value to the same key.
    let result = env.transact_manual(|tx| {
        let frame = tx.call(&ADDR).run(|| bytestring_set(&KEY_A, b"second"));
        frame.assert_success();
        tx.try_commit_transaction()
    });

    let conflicts = conflicts_of(result);
    assert_eq!(conflicts.len(), 1, "expected exactly one conflicting slot");
    assert_eq!(conflicts[0].code, DbConflictCode::ValueMismatch);
    assert_eq!(conflicts[0].addrkey.address, ADDR);
}

#[test]
fn bytestring_two_conflicts_returns_all() {
    let env = TestEnvironment::no_cost();

    // First writer touches both keys and commits cleanly.
    env.transact(|tx| {
        tx.call(&ADDR).run(|| {
            bytestring_set(&KEY_A, b"first-a");
            bytestring_set(&KEY_B, b"first-b");
        })
    })
    .assert_success();

    // Second writer collides on both keys in the same block.
    let result = env.transact_manual(|tx| {
        let frame = tx.call(&ADDR).run(|| {
            bytestring_set(&KEY_A, b"second-a");
            bytestring_set(&KEY_B, b"second-b");
        });
        frame.assert_success();
        tx.try_commit_transaction()
    });

    let conflicts = conflicts_of(result);
    assert_eq!(
        conflicts.len(),
        2,
        "expected both conflicting slots reported"
    );
    assert!(
        conflicts
            .iter()
            .all(|c| c.code == DbConflictCode::ValueMismatch),
        "all conflicts should be value mismatches: {conflicts:?}"
    );
    assert!(
        conflicts.iter().all(|c| c.addrkey.address == ADDR),
        "all conflicts should be for the written contract",
    );
    // Both writes hit distinct keys, so the two diagnostics must be distinct.
    let mut slots: Vec<String> = conflicts
        .iter()
        .map(|c| format!("{:?}", c.addrkey.key))
        .collect();
    slots.sort();
    slots.dedup();
    assert_eq!(slots.len(), 2, "the two conflicting slots must be distinct");
}
