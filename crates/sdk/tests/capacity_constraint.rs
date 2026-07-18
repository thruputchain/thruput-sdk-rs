//! Test the semantics of the `CapacityConstraint` storage type: staged
//! consumption, drop-time merging across handles, and the set-capacity
//! first-write rule.

use sdk::contract_test;
use sdk::exn::LibraryExn;
use sdk::storage::*;

contract_test!(test_consumption_merges_across_handles {
    // Two sequential handles to the same cell within one transaction: the
    // second handle's writeback must merge with (not replace) the first's.
    {
        let mut c: CapacityConstraint = Keypath::default().into();
        c.set_capacity(100);
        c.consume_capacity(60);
        assert_eq!(c.get_remaining_capacity(), 40);
    }
    {
        let mut c: CapacityConstraint = Keypath::default().into();
        c.consume_capacity(30);
        assert_eq!(c.get_remaining_capacity(), 10);
    }
    let c: CapacityConstraint = Keypath::default().into();
    assert_eq!(c.get_remaining_capacity(), 10);
});

contract_test!(test_negative_consume_releases_capacity {
    let mut c: CapacityConstraint = Keypath::default().into();
    c.set_capacity(10);
    c.consume_capacity(7);
    c.consume_capacity(-3);
    assert_eq!(c.get_remaining_capacity(), 6);
});

contract_test!(test_temporary_violation_within_transaction {
    let mut c: CapacityConstraint = Keypath::default().into();
    c.set_capacity(10);
    c.consume_capacity(15);
    assert_eq!(c.get_remaining_capacity(), -5);
    c.consume_capacity(-8);
    assert_eq!(c.get_remaining_capacity(), 3);
});

contract_test!(test_set_capacity_after_local_write_reports_err {
    let mut c: CapacityConstraint = Keypath::default().into();
    c.consume_capacity(5);
    assert_eq!(
        c.try_set_capacity(100),
        Err(LibraryExn::CapacityConstraintAlreadyWritten)
    );

    let mut c2: CapacityConstraint = Keypath::default().into();
    c2.set_capacity(100);
    assert_eq!(
        c2.try_set_capacity(50),
        Err(LibraryExn::CapacityConstraintAlreadyWritten)
    );
});

contract_test!(test_set_capacity_after_dropped_write_reports_err {
    // The first-write check must consult the pending storage state, so it
    // catches writes flushed by a handle that has already been dropped.
    {
        let mut c: CapacityConstraint = Keypath::default().into();
        c.consume_capacity(5);
    }
    let mut c: CapacityConstraint = Keypath::default().into();
    assert_eq!(
        c.try_set_capacity(100),
        Err(LibraryExn::CapacityConstraintAlreadyWritten)
    );
    // Release the consumption so the surrounding transaction can commit
    // (the cell was never given capacity).
    c.consume_capacity(-5);
});

contract_test!(test_read_only_handle_leaves_no_trace {
    {
        // A never-written cell reads as zero remaining capacity.
        let c: CapacityConstraint = Keypath::default().into();
        assert_eq!(c.get_remaining_capacity(), 0);
    }
    // The read-only handle above must not count as a prior write.
    let mut c: CapacityConstraint = Keypath::default().into();
    assert_eq!(c.try_set_capacity(100), Ok(()));
    assert_eq!(c.get_remaining_capacity(), 100);
});
