//! Test "storage" datatypes: keypaths, handles, vectors and maps

use thruput_sdk::contract_test;
use thruput_sdk::exn::NoneIfMissing;
use thruput_sdk::storage::*;
use thruput_sdk::{Deletable, Storage};

#[derive(Storage, Deletable)]
#[allow(dead_code)]
struct Pair<A: StorageType, B: StorageType> {
    first: A,
    second: B,
}

#[derive(Debug, Storage)]
#[allow(dead_code)]
struct Data {
    num: Value<i32>,
    nums: Vector<Value<u64>>,
    nnints: Vector<NNInt>,
    bytes: Bytestring,
    map: Map<u32, NNInt>,
}

#[derive(Storage)]
struct InnerValuePair {
    first: Value<u32>,
    second: Value<u64>,
}

contract_test!(test_map {
    let data_keypath = Keypath::default();
    let slot: u32 = 3;
    {
        let mut data: Data = data_keypath.clone().into();
        data.map.at_mut(&slot).strict_inc(7);
    }

    {
        let data: Data = data_keypath.into();
        let v = data.map.at(&slot).get();
        assert_eq!(v, 7);
    }
});

contract_test!(test_map_entry {
    let mut map: Map<u32, Value<i32>> = Keypath::default().into();

    let slot: u32 = 3;

    // Check that the entry starts empty
    {
        let v = map.at(&slot).try_get().none_if_missing();
        assert_eq!(v, None);
    }

    let mut value: i32 = -14;

    // Set the entry via at_mut().set()
    {
        map.at_mut(&slot).set(&value);
    }

    // Check that the entry has the value
    {
        let v = map.at(&slot).try_get().none_if_missing();
        assert_eq!(v, Some(value));
    }

    value += 1;

    // Set via at_mut().set() again
    {
        map.at_mut(&slot).set(&value);
    }

    // Check
    {
        let v = map.at(&slot).try_get().none_if_missing();
        assert_eq!(v, Some(value));
    }

    value += 1;

    // Set via load_mut
    {
        let mut slot_mut = map.at_mut(&slot);
        let mut data = slot_mut.load_mut();
        *data = value;
    }

    // Check
    {
        let v = map.at(&slot).try_get().none_if_missing();
        assert_eq!(v, Some(value));
    }
});

contract_test!(test_storable {
    let data_keypath = Keypath::default();
    {
        let mut data: Data = data_keypath.clone().into();
        for i in 0..3u64 {
            data.nums.push_with(|slot| slot.set(&i));
        }
    }

    let data: Data = data_keypath.into();
    for i in 0..3u32 {
        let n = data.nums.at(i).get();
        assert_eq!(n, i as u64);
    }
});

contract_test!(test_vector {
    let mut vec: Vector<Value<u32>> = Keypath::default().into();

    assert_eq!(0, vec.len());
    for i in 0..3u32 {
        vec.push_empty_slot().set(&i);
    }

    assert_eq!(3, vec.len());
    for i in 0..3u32 {
        assert_eq!(i, vec.at(i).get());
    }
});

contract_test!(test_vector_push_with {
    let mut vec: Vector<Value<u32>> = Keypath::default().into();

    vec.push_with(|slot| slot.set(&10));
    vec.push_with(|slot| slot.set(&20));

    assert_eq!(vec.len(), 2);
    assert_eq!(vec.at(0).get(), 10);
    assert_eq!(vec.at(1).get(), 20);
});

contract_test!(test_vector_push_with_nested_node {
    let mut vec: Vector<Node<InnerValuePair>> = Keypath::default().into();

    vec.push_with(|elem| {
        elem.first.set(&7);
        elem.second.set(&11);
    });

    assert_eq!(vec.len(), 1);
    assert_eq!(vec.at(0).first.get(), 7);
    assert_eq!(vec.at(0).second.get(), 11);
});

contract_test!(test_map_at_nested {
    let mut map: Map<u32, Node<InnerValuePair>> = Keypath::default().into();
    let slot: u32 = 5;
    {
        let mut entry = map.at_mut(&slot);
        entry.first.set(&7);
        entry.second.set(&11);
    }

    {
        let entry = map.at(&slot);
        assert_eq!(entry.first.get(), 7);
        assert_eq!(entry.second.get(), 11);
    }
});

contract_test!(test_value_basic {
    let mut val: Value<i32> = Keypath::default().into();

    // Initially empty
    assert_eq!(val.try_get().none_if_missing(), None);

    // Set a value
    val.set(&42);
    assert_eq!(val.get(), 42);

    // Overwrite
    val.set(&99);
    assert_eq!(val.get(), 99);

    // Delete
    val.delete();
    assert_eq!(val.try_get().none_if_missing(), None);
});

contract_test!(test_value_load_mut {
    let mut val: Value<i32> = Keypath::default().into();

    // Set initial value
    val.set(&10);

    // Modify via load_mut — changes flush on drop
    {
        let mut data = val.load_mut();
        *data = 42;
    }

    assert_eq!(val.get(), 42);
});

contract_test!(test_value_mut_flush {
    let mut val: Value<i32> = Keypath::default().into();
    val.set(&1);

    let mut guard = val.load_mut();
    // A clean guard flushes as a no-op.
    assert_eq!(guard.try_flush(), Ok(()));

    // An explicit flush writes immediately, before drop.
    *guard = 5;
    guard.flush();
    let peek: Value<i32> = Keypath::default().into();
    assert_eq!(peek.get(), 5);

    // The flushed guard is clean but still usable: mutating it re-arms
    // the drop-time writeback.
    *guard = 6;
    drop(guard);
    assert_eq!(val.get(), 6);

    // A guard that is never mutated writes nothing at drop.
    {
        let guard = val.load_mut();
        assert_eq!(*guard, 6);
    }
    assert_eq!(val.get(), 6);
});

contract_test!(test_map_at_value {
    let mut map: Map<u32, Value<i32>> = Keypath::default().into();

    // Write via at_mut
    map.at_mut(&1u32).set(&100);

    // Read via at
    let v = map.at(&1u32).try_get().none_if_missing();
    assert_eq!(v, Some(100));

    // Modify via load_mut through at_mut
    {
        let mut slot = map.at_mut(&1u32);
        let mut data = slot.load_mut();
        *data = 200;
    }

    let v = map.at(&1u32).get();
    assert_eq!(v, 200);
});

contract_test!(test_exclusive_propagation {
    let data_keypath = Keypath::default();
    let slot: u32 = 7;

    // Use at_mut_exclusive -- exclusive flag reaches NNInt
    {
        let mut data: Data = data_keypath.clone().into();
        data.map.at_mut_exclusive(&slot).strict_inc(10);
    }

    {
        let data: Data = data_keypath.into();
        let v = data.map.at(&slot).get();
        assert_eq!(v, 10);
    }
});

contract_test!(test_exclusive_write_keeps_its_lock {
    use thruput_sdk::StorageFlags;

    let mut map: Map<u32, Value<u32>> = Keypath::default().into();
    let slot: u32 = 5;

    // Building or reading a handle requests nothing.
    assert_eq!(map.at(&slot).flags(), StorageFlags::None);
    map.at_mut_exclusive(&slot).set(&1);
    assert_eq!(map.at(&slot).flags(), StorageFlags::ExclusiveLock);

    // Writes and deletes through ordinary handles keep the lock.
    map.at_mut(&slot).set(&2);
    map.at_mut(&slot).delete();
    map.at_mut(&slot).set(&3);
    assert_eq!(map.at(&slot).flags(), StorageFlags::ExclusiveLock);

    // Clearing drops the lock, not the write.
    map.at_mut(&slot).clear_flags();
    assert_eq!(map.at(&slot).flags(), StorageFlags::None);
    assert_eq!(map.at(&slot).get(), 3);
});

contract_test!(test_set_flags_needs_a_pending_write {
    use thruput_sdk::StorageFlags;
    use thruput_sdk::sys_result::flags::set_storage_flags;

    let mut value: Value<u32> = Keypath::default().child(&1u32).into();

    // Nothing written yet: flags have nothing to attach to, and clearing
    // does nothing.
    assert!(set_storage_flags(value.key(), StorageFlags::ExclusiveLock).is_err());
    value.clear_flags();
    assert_eq!(value.flags(), StorageFlags::None);

    value.set(&7);
    value.set_flags(StorageFlags::ExclusiveLock);
    // set_flags adds: adding None removes nothing.
    value.set_flags(StorageFlags::None);
    assert_eq!(value.flags(), StorageFlags::ExclusiveLock);
});

contract_test!(test_exclusive_delete_locks_the_deletion {
    use thruput_sdk::StorageFlags;

    let mut count: NNInt = Keypath::default().child(&2u32).into();
    count.set(4);
    assert_eq!(count.flags(), StorageFlags::None);
    count.delete_exclusive();
    assert_eq!(count.flags(), StorageFlags::ExclusiveLock);
    count.inc(1);
    assert_eq!(count.flags(), StorageFlags::ExclusiveLock);
});

contract_test!(test_value_set_exclusive {
    let mut val: Value<u32> = Keypath::default().into();

    val.set_exclusive(&10);
    assert_eq!(val.get(), 10);

    assert_eq!(val.try_set_exclusive(&20), Ok(()));
    assert_eq!(val.get(), 20);
});

contract_test!(test_vector_pop {
    let mut vec: Vector<Value<u32>> = Keypath::default().into();

    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);
    vec.push_empty_slot().set(&30);
    assert_eq!(3, vec.len());

    vec.pop();
    assert_eq!(2, vec.len());

    // Popped element's storage is deleted
    assert_eq!(vec.at(0).get(), 10);
    assert_eq!(vec.at(1).get(), 20);

    vec.pop();
    vec.pop();
    assert_eq!(0, vec.len());
});

contract_test!(test_vector_delete {
    let mut vec: Vector<Value<u32>> = Keypath::default().into();

    vec.push_empty_slot().set(&1);
    vec.push_empty_slot().set(&2);
    vec.push_empty_slot().set(&3);

    vec.delete();
    assert_eq!(0, vec.len());
});

contract_test!(test_vector_delete_verifies_storage_cleared {
    let root = Keypath::default();
    let mut vec: Vector<Value<u32>> = root.clone().into();

    vec.push_empty_slot().set(&100);
    vec.push_empty_slot().set(&200);
    vec.push_empty_slot().set(&300);

    // Verify values are there via manually-constructed keypaths.
    // Vector layout:
    //   len       = root.child(&0u32)
    //   values map = root.child(&1u32)
    //   element i  = values_kp.child(&i)
    let values_kp = root.child(&1u32);

    let elem = |kp: &Keypath, idx: u32| -> Value<u32> {
        kp.child(&idx).into()
    };

    // Before delete: elements are readable
    assert_eq!(elem(&values_kp, 0).try_get().none_if_missing(), Some(100));
    assert_eq!(elem(&values_kp, 1).try_get().none_if_missing(), Some(200));
    assert_eq!(elem(&values_kp, 2).try_get().none_if_missing(), Some(300));

    // Delete the whole vector
    vec.delete();

    // After delete: all element storage is gone
    assert_eq!(elem(&values_kp, 0).try_get().none_if_missing(), None);
    assert_eq!(elem(&values_kp, 1).try_get().none_if_missing(), None);
    assert_eq!(elem(&values_kp, 2).try_get().none_if_missing(), None);

    // Length storage is also gone
    let len_kp = root.child(&0u32);
    let len_val: Value<u32> = len_kp.into();
    assert_eq!(len_val.try_get().none_if_missing(), None);
});

contract_test!(test_generic_storage_struct {
    let keypath = Keypath::default();
    let pair: Pair<Value<u32>, Value<u64>> = keypath.into();

    let mut first = pair.first;
    let mut second = pair.second;

    first.set(&42);
    second.set(&100);

    assert_eq!(first.get(), 42);
    assert_eq!(second.get(), 100);
});

contract_test!(test_generic_storage_struct_delete {
    let keypath = Keypath::default();
    let mut pair: Pair<Value<u32>, Value<u64>> = keypath.into();

    pair.first.set(&42);
    pair.second.set(&100);

    pair.delete();

    assert_eq!(pair.first.try_get().none_if_missing(), None);
    assert_eq!(pair.second.try_get().none_if_missing(), None);
});

contract_test!(test_nnint_delete {
    let keypath = Keypath::default();
    let mut n: NNInt = keypath.into();
    n.set(42);
    assert_eq!(n.get(), 42);
    n.delete();
    assert_eq!(n.try_get().none_if_missing(), None);
});

contract_test!(test_vector_pop_nnint {
    let keypath = Keypath::default();
    let mut vec: Vector<NNInt> = keypath.into();

    vec.push_empty_slot().set(10);
    vec.push_empty_slot().set(20);
    assert_eq!(2, vec.len());

    vec.pop();
    assert_eq!(1, vec.len());
    assert_eq!(vec.at(0).get(), 10);
});

contract_test!(test_vector_iter {
    let mut vec: Vector<Value<u32>> = Keypath::default().into();

    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);
    vec.push_empty_slot().set(&30);

    let values: Vec<u32> = vec.iter().map(|v| v.get()).collect();
    assert_eq!(values, [10, 20, 30]);
});

contract_test!(test_vector_iter_mut {
    let mut vec: Vector<NNInt> = Keypath::default().into();

    vec.push_empty_slot().set(10);
    vec.push_empty_slot().set(20);
    vec.push_empty_slot().set(30);

    for mut elem in vec.iter_mut() {
        elem.strict_inc(1);
    }

    let values: Vec<i128> = vec.iter().map(|v| v.get()).collect();
    assert_eq!(values, [11, 21, 31]);
});

contract_test!(test_vector_iter_rev {
    let mut vec: Vector<Value<u32>> = Keypath::default().into();

    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);
    vec.push_empty_slot().set(&30);

    let values: Vec<u32> = vec.iter().rev().map(|v| v.get()).collect();
    assert_eq!(values, [30, 20, 10]);
});

contract_test!(test_vector_into_iter {
    let mut vec: Vector<Value<u32>> = Keypath::default().into();

    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);

    // for elem in &vec
    let mut sum = 0u32;
    for elem in &vec {
        sum += elem.get();
    }
    assert_eq!(sum, 30);

    // for mut elem in &mut vec
    for mut elem in &mut vec {
        let v = elem.get();
        elem.set(&(v + 1));
    }

    assert_eq!(vec.at(0).get(), 11);
    assert_eq!(vec.at(1).get(), 21);
});

contract_test!(test_vector_iter_empty {
    let vec: Vector<Value<u32>> = Keypath::default().into();
    assert_eq!(vec.iter().count(), 0);
});

// (This test environment uses the default configured bytestring limit of
// 1024 bytes, below the SDK's read scratch, so a stored value too large for
// the read buffer can't be constructed here — the undecodable-value tests below
// cover the StorageSerialization surface instead.)

#[test]
fn test_value_undecodable_read_reports_err() {
    // Write a raw bytestring that is not a valid encoding of u32 directly
    // at the storage key used by a Value<u32>. try_get reports
    // StorageSerialization as an Err — not a storage-missing error.
    let env = thruput_sdk::TestEnvironment::no_cost();
    let addr = <thruput_sdk::ContractAddress as thruput_sdk::Named>::named(
        "test_value_undecodable_read_reports_err",
    );
    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let keypath = Keypath::default();
            let key = keypath.generate_key();
            thruput_sdk::bytestring_set(&key, &[0u8; 900]);
            let val: Value<u32> = keypath.into();
            let err = val.try_get().unwrap_err();
            assert!(!err.is_storage_missing());
            assert_eq!(
                err,
                thruput_sdk::exn::LibraryExn::StorageSerialization
            );
        })
    });
    assert!(result.is_success());
}

#[test]
fn test_value_undecodable_get_aborts() {
    // The plain get() on the same undecodable slot aborts with exactly the
    // exception try_get reports.
    let env = thruput_sdk::TestEnvironment::no_cost();
    let addr = <thruput_sdk::ContractAddress as thruput_sdk::Named>::named(
        "test_value_undecodable_get_aborts",
    );
    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let keypath = Keypath::default();
            let key = keypath.generate_key();
            thruput_sdk::bytestring_set(&key, &[0u8; 900]);
            let val: Value<u32> = keypath.into();
            let _ = val.get();
        })
    });
    assert!(!result.is_success());
}

contract_test!(test_map_at_disjoint_mut {
    let mut map: Map<u32, NNInt> = Keypath::default().into();

    map.at_mut(&1u32).set(100);
    map.at_mut(&2u32).set(200);
    map.at_mut(&3u32).set(300);

    {
        let mut slots = map.at_disjoint_mut([&1u32, &2u32, &3u32]);
        slots[0].strict_inc(1);
        slots[1].strict_inc(2);
        slots[2].strict_inc(3);
    }

    assert_eq!(map.at(&1u32).get(), 101);
    assert_eq!(map.at(&2u32).get(), 202);
    assert_eq!(map.at(&3u32).get(), 303);
});

#[test]
fn test_map_at_disjoint_mut_duplicate_panics() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let addr = <thruput_sdk::ContractAddress as thruput_sdk::Named>::named(
        "test_map_at_disjoint_mut_duplicate_panics",
    );
    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let mut map: Map<u32, NNInt> = Keypath::default().into();
            map.at_disjoint_mut([&1u32, &1u32]);
        })
    });
    assert!(!result.is_success());
}

#[derive(thruput_sdk::Storage)]
struct StorageDomainPlain {
    _v: thruput_sdk::storage::Value<u64>,
}

#[thruput_sdk::storage(domain = "custom")]
struct StorageDomainRenamed {
    _v: thruput_sdk::storage::Value<u64>,
}

contract_test!(storage_domain_defaults_to_ident_and_overrides {
    // Behavioral: the domain seeds each root field path as its first child
    // frame. Default is the bare ident; #[storage(domain = "...")] overrides.
    let plain = contract_state_root::<StorageDomainPlain>();
    assert_eq!(
        *plain._v.key(),
        Keypath::default()
            .child("StorageDomainPlain")
            .child(&0u32)
            .generate_key()
    );
    let renamed = contract_state_root::<StorageDomainRenamed>();
    assert_eq!(
        *renamed._v.key(),
        Keypath::default().child("custom").child(&0u32).generate_key()
    );
});

#[test]
fn test_data_fully_consumed() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let addr = <thruput_sdk::ContractAddress as thruput_sdk::Named>::named(
        "test_data_fully_consumed",
    );

    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let keypath = Keypath::default();
            {
                let mut value: Value<(i32, i32)> = keypath.clone().into();
                value.set(&(10, 20));
            }

            {
                // check that the value is read correctly
                let value: Value<(i32, i32)> = keypath.clone().into();
                let (a, b) = value.get();
                assert_eq!(a, 10);
                assert_eq!(b, 20);
            }

            {
                // Decoding (i32, i32) data as i32 leaves bytes unconsumed,
                // so try_get reports StorageSerialization (and get() would
                // abort with the same exception).
                let value: Value<i32> = keypath.clone().into();
                assert_eq!(
                    value.try_get(),
                    Err(thruput_sdk::exn::LibraryExn::StorageSerialization)
                );
            }
        })
    });
    assert!(result.is_success());
}

#[derive(thruput_sdk::Storage)]
#[allow(dead_code)]
struct Pinned {
    a: Value<u64>, // positional 0
    #[slot(7)]
    b: Value<u64>, // pinned 7
}

contract_test!(slot_override_remaps_field_keypath {
    let root = Keypath::default();
    let p = Pinned::from(root.clone());
    // b lives at child(&7u32), provably distinct from child(&1u32).
    let expect_b = Value::<u64>::from(root.child(&7u32));
    let other = Value::<u64>::from(root.child(&1u32));
    assert_eq!(p.b.key(), expect_b.key());
    assert_ne!(p.b.key(), other.key());
});

#[derive(thruput_sdk::Storage)]
#[allow(dead_code)]
struct PositionalAbsoluteIndex {
    #[slot(5)]
    a: Value<u64>, // pinned 5
    b: Value<u64>, // positional: declaration index 1
    c: Value<u64>, // positional: declaration index 2
}

contract_test!(positional_fields_use_absolute_declaration_index {
    let root = Keypath::default();
    let s = PositionalAbsoluteIndex::from(root.clone());
    // c is the 3rd field -> child(&2u32), NOT child(&1u32).
    assert_eq!(s.c.key(), Value::<u64>::from(root.child(&2u32)).key());
    assert_ne!(s.c.key(), Value::<u64>::from(root.child(&1u32)).key());
    // b -> child(&1u32); a -> child(&5u32).
    assert_eq!(s.b.key(), Value::<u64>::from(root.child(&1u32)).key());
    assert_eq!(s.a.key(), Value::<u64>::from(root.child(&5u32)).key());
});

// ===========================================================================
// `Deletable for SlotMut`: deleting storage through a borrowed slot.
//
// `SlotMut` forwards `delete_internal` to the slot it wraps, so a borrowed
// entry deletes exactly like an owned handle to the same keypath — and a
// `SlotMut` now satisfies a `T: Deletable` bound, which is what makes
// `Map::delete_at`, `Vector::pop` over borrowed elements, and generic
// delete helpers possible.
// ===========================================================================

/// A `Deletable` node type for exercising recursive deletes through a slot.
#[derive(Storage, Deletable)]
struct DeletablePair {
    first: Value<u32>,
    second: Value<u64>,
}

contract_test!(test_map_slot_delete_leaves_siblings {
    let mut map: Map<u32, Value<i32>> = Keypath::default().into();

    map.at_mut(&1u32).set(&10);
    map.at_mut(&2u32).set(&20);

    // Delete through the borrowed slot.
    map.at_mut(&1u32).delete();

    assert_eq!(map.at(&1u32).try_get().none_if_missing(), None);
    // Only the addressed entry is affected.
    assert_eq!(map.at(&2u32).try_get().none_if_missing(), Some(20));

    // The slot is reusable: a deleted entry is indistinguishable from one
    // that was never written, and writing it again just works.
    map.at_mut(&1u32).set(&11);
    assert_eq!(map.at(&1u32).try_get().none_if_missing(), Some(11));
});

contract_test!(test_map_delete_at {
    let mut map: Map<u32, Value<i32>> = Keypath::default().into();

    map.at_mut(&1u32).set(&10);
    map.at_mut(&2u32).set(&20);

    map.delete_at(&1u32);

    assert_eq!(map.at(&1u32).try_get().none_if_missing(), None);
    assert_eq!(map.at(&2u32).try_get().none_if_missing(), Some(20));
});

contract_test!(test_map_delete_at_absent_is_a_noop {
    // Deleting a never-written entry is not an error (same contract as
    // `delete_shared` on an unstored key), and it leaves the map alone.
    let mut map: Map<u32, Value<i32>> = Keypath::default().into();
    map.at_mut(&1u32).set(&10);

    map.delete_at(&99u32);
    map.at_mut(&99u32).delete();

    assert_eq!(map.at(&99u32).try_get().none_if_missing(), None);
    assert_eq!(map.at(&1u32).try_get().none_if_missing(), Some(10));

    // And deleting twice is likewise fine.
    map.delete_at(&1u32);
    map.delete_at(&1u32);
    assert_eq!(map.at(&1u32).try_get().none_if_missing(), None);
});

contract_test!(test_map_slot_delete_nnint {
    // Deleting an NNInt entry clears `set` and the accumulated `add`: the
    // slot reads back as missing, and a later `strict_inc` starts from 0
    // rather than resuming the pre-delete total.
    let mut map: Map<u32, NNInt> = Keypath::default().into();

    map.at_mut(&1u32).set(100);
    map.at_mut(&1u32).strict_inc(5);
    assert_eq!(map.at(&1u32).get(), 105);

    map.delete_at(&1u32);
    assert_eq!(map.at(&1u32).try_get().none_if_missing(), None);

    map.at_mut(&1u32).strict_inc(7);
    assert_eq!(map.at(&1u32).get(), 7);
});

contract_test!(test_map_slot_delete_is_recursive_through_node {
    // `Node<T>` delegates to `T`'s derived `Deletable`, so deleting the
    // entry slot clears every field of the struct behind it.
    let mut map: Map<u32, Node<DeletablePair>> = Keypath::default().into();

    {
        let mut entry = map.at_mut(&1u32);
        entry.first.set(&7);
        entry.second.set(&11);
    }
    assert_eq!(map.at(&1u32).first.try_get().none_if_missing(), Some(7));

    map.delete_at(&1u32);

    assert_eq!(map.at(&1u32).first.try_get().none_if_missing(), None);
    assert_eq!(map.at(&1u32).second.try_get().none_if_missing(), None);
});

contract_test!(test_map_slot_delete_is_recursive_through_vector {
    // A `Vector` entry deletes its elements and its len slot, so the entry
    // reads back as an empty vector.
    let mut map: Map<u32, Vector<Value<u64>>> = Keypath::default().into();

    {
        let mut v = map.at_mut(&1u32);
        v.push_empty_slot().set(&7);
        v.push_empty_slot().set(&8);
    }
    {
        let mut v = map.at_mut(&2u32);
        v.push_empty_slot().set(&9);
    }
    assert_eq!(map.at(&1u32).len(), 2);

    map.delete_at(&1u32);

    assert_eq!(map.at(&1u32).len(), 0);
    assert!(map.at(&1u32).is_empty());
    // The sibling entry's vector is untouched.
    assert_eq!(map.at(&2u32).len(), 1);
    assert_eq!(map.at(&2u32).at(0).get(), 9);
});

contract_test!(test_slot_mut_satisfies_deletable_bound {
    // The point of the impl: a borrowed slot can be handed to code that is
    // generic over `Deletable`. Without `impl Deletable for SlotMut` these
    // calls would not resolve.
    fn erase<D: Deletable>(d: &mut D) {
        d.delete();
    }
    fn erase_exclusive<D: Deletable>(d: &mut D) {
        d.delete_exclusive();
    }

    let mut map: Map<u32, Value<i32>> = Keypath::default().into();
    map.at_mut(&1u32).set(&10);
    map.at_mut(&2u32).set(&20);

    erase(&mut map.at_mut(&1u32));
    erase_exclusive(&mut map.at_mut(&2u32));

    assert_eq!(map.at(&1u32).try_get().none_if_missing(), None);
    assert_eq!(map.at(&2u32).try_get().none_if_missing(), None);

    // Also through a `Vector` element slot.
    let mut vec: Vector<Value<u32>> = Keypath::default().child("vec").into();
    vec.push_empty_slot().set(&1);
    erase(&mut vec.at_mut(0));
    assert_eq!(vec.at(0).try_get().none_if_missing(), None);
});

contract_test!(test_map_slot_delete_addresses_the_same_key_as_an_owned_handle {
    // A slot delete must hit exactly the key the map derives for that entry
    // — not, say, the map root.
    let root = Keypath::default();
    let mut map: Map<u32, Value<i32>> = root.clone().into();
    map.at_mut(&1u32).set(&10);

    // Independently derive the entry's keypath and confirm it sees the write.
    let mut owned: Value<i32> = root.child(&1u32).into();
    assert_eq!(owned.try_get().none_if_missing(), Some(10));

    map.at_mut(&1u32).delete();
    assert_eq!(owned.try_get().none_if_missing(), None);

    // ...and the converse: deleting the owned handle clears the map entry.
    map.at_mut(&1u32).set(&20);
    owned.delete();
    assert_eq!(map.at(&1u32).try_get().none_if_missing(), None);
});

// ---------------------------------------------------------------------------
// Vector elements: deleting through a slot punches a hole.
//
// `Vector` tracks a `len` and nothing else — there is no per-element
// occupancy bit and no way to shift elements down. Deleting element `i`
// therefore removes only that element's storage: `len` is unchanged, `i`
// stays in bounds, and the slot reads back as missing. `len` is a
// high-water mark, not a count of live elements. These tests pin that
// behavior down, including the sharp edges.
// ---------------------------------------------------------------------------

contract_test!(test_vector_element_delete_punches_a_hole {
    let mut vec: Vector<Value<u32>> = Keypath::default().into();
    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);
    vec.push_empty_slot().set(&30);

    vec.at_mut(1).delete();

    // len does NOT shrink, and the index stays in bounds.
    assert_eq!(vec.len(), 3);
    assert!(!vec.is_empty());

    // Neighbours do not shift down.
    assert_eq!(vec.at(0).try_get().none_if_missing(), Some(10));
    assert_eq!(vec.at(1).try_get().none_if_missing(), None);
    assert_eq!(vec.at(2).try_get().none_if_missing(), Some(30));

    // The hole is refillable in place.
    vec.at_mut(1).set(&21);
    assert_eq!(vec.at(1).try_get().none_if_missing(), Some(21));
    assert_eq!(vec.len(), 3);
});

#[test]
fn test_vector_get_on_a_hole_aborts() {
    // `at(i)` is only bounds-checked; it does not check occupancy. Reading a
    // deleted in-bounds element with `get()` aborts as storage-missing, so
    // callers that delete elements must read with `try_get`.
    let env = thruput_sdk::TestEnvironment::no_cost();
    let addr = <thruput_sdk::ContractAddress as thruput_sdk::Named>::named(
        "test_vector_get_on_a_hole_aborts",
    );
    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let mut vec: Vector<Value<u32>> = Keypath::default().into();
            vec.push_empty_slot().set(&10);
            vec.push_empty_slot().set(&20);

            vec.at_mut(0).delete();

            // The error is storage-missing, not out-of-bounds.
            let err = vec.at(0).try_get().unwrap_err();
            assert!(err.is_storage_missing());

            // ...and `get()` aborts with it.
            let _ = vec.at(0).get();
        })
    });
    assert!(!result.is_success());
}

contract_test!(test_vector_iter_over_holes {
    // `iter()` walks 0..len and so yields the holes too. A consumer that
    // calls `get()` would abort; `try_get().none_if_missing()` is the
    // hole-tolerant read.
    let mut vec: Vector<Value<u32>> = Keypath::default().into();
    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);
    vec.push_empty_slot().set(&30);

    vec.at_mut(1).delete();

    let seen: Vec<Option<u32>> =
        vec.iter().map(|v| v.try_get().none_if_missing()).collect();
    assert_eq!(seen, [Some(10), None, Some(30)]);

    // The iterator length still reflects len, not the live-element count.
    assert_eq!(vec.iter().count(), 3);
    assert_eq!(vec.iter().len(), 3);

    let live: Vec<u32> =
        vec.iter().filter_map(|v| v.try_get().none_if_missing()).collect();
    assert_eq!(live, [10, 30]);
});

contract_test!(test_vector_iter_mut_delete_all_keeps_len {
    // Deleting every element through `iter_mut()` empties the storage but
    // leaves `len` intact — the vector is all holes, not empty. Use
    // `Vector::delete` to clear len as well.
    let mut vec: Vector<Value<u32>> = Keypath::default().into();
    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);

    for mut elem in vec.iter_mut() {
        elem.delete();
    }

    assert_eq!(vec.len(), 2);
    assert!(!vec.is_empty());
    assert_eq!(vec.at(0).try_get().none_if_missing(), None);
    assert_eq!(vec.at(1).try_get().none_if_missing(), None);

    // `delete()` is the way to actually reset the vector.
    vec.delete();
    assert_eq!(vec.len(), 0);
    assert!(vec.is_empty());
});

contract_test!(test_vector_pop_tolerates_holes {
    // `pop` deletes the last element's storage and decrements len. Deleting
    // storage that is already gone is a no-op, so popping over holes works
    // and len still lands on 0.
    let mut vec: Vector<Value<u32>> = Keypath::default().into();
    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);
    vec.push_empty_slot().set(&30);

    // Punch a hole at the tail, then pop it.
    vec.at_mut(2).delete();
    vec.pop();
    assert_eq!(vec.len(), 2);
    assert_eq!(vec.at(1).try_get().none_if_missing(), Some(20));

    // Punch one in the middle and pop past it.
    vec.at_mut(1).delete();
    vec.pop();
    vec.pop();
    assert_eq!(vec.len(), 0);
});

contract_test!(test_vector_delete_tolerates_holes {
    // `Vector::delete` walks 0..len deleting each element; holes are
    // no-ops, and the len slot is deleted too.
    let root = Keypath::default();
    let mut vec: Vector<Value<u32>> = root.clone().into();
    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);
    vec.push_empty_slot().set(&30);

    vec.at_mut(1).delete();
    vec.delete();

    assert_eq!(vec.len(), 0);

    // Element storage is gone, checked through independently-derived
    // keypaths (vector layout: len = child(0), values map = child(1)).
    let values_kp = root.child(&1u32);
    for i in 0..3u32 {
        let elem: Value<u32> = values_kp.child(&i).into();
        assert_eq!(elem.try_get().none_if_missing(), None);
    }
});

contract_test!(test_vector_push_after_hole_appends_at_len {
    // A hole does not become a free slot: `push` still appends at `len`,
    // so the hole persists and len keeps growing.
    let mut vec: Vector<Value<u32>> = Keypath::default().into();
    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);

    vec.at_mut(0).delete();
    vec.push_with(|slot| slot.set(&30));

    assert_eq!(vec.len(), 3);
    assert_eq!(vec.at(0).try_get().none_if_missing(), None);
    assert_eq!(vec.at(1).try_get().none_if_missing(), Some(20));
    assert_eq!(vec.at(2).try_get().none_if_missing(), Some(30));
});

contract_test!(test_vector_nnint_hole_resurrects_at_zero {
    // The sharp edge worth knowing about: for `Vector<NNInt>` a hole is
    // NOT durably absent. `strict_inc` treats a missing location as 0, so
    // incrementing a deleted element silently recreates it — a deleted
    // element and a zero element are indistinguishable to a writer.
    let mut vec: Vector<NNInt> = Keypath::default().into();
    vec.push_empty_slot().set(100);
    vec.push_empty_slot().set(200);

    vec.at_mut(0).delete();
    assert_eq!(vec.at(0).try_get().none_if_missing(), None);

    vec.at_mut(0).strict_inc(5);
    assert_eq!(vec.at(0).try_get().none_if_missing(), Some(5));
    assert_eq!(vec.at(1).get(), 200);
});

#[test]
fn test_vector_element_delete_is_still_bounds_checked() {
    // `at_mut` bounds-checks before handing out the slot, so an
    // out-of-bounds delete aborts rather than clearing an unrelated key.
    let env = thruput_sdk::TestEnvironment::no_cost();
    let addr = <thruput_sdk::ContractAddress as thruput_sdk::Named>::named(
        "test_vector_element_delete_is_still_bounds_checked",
    );
    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let mut vec: Vector<Value<u32>> = Keypath::default().into();
            vec.push_empty_slot().set(&10);
            vec.at_mut(5).delete();
        })
    });
    assert!(!result.is_success());
}

contract_test!(test_vector_delete_at_matches_slot_delete {
    // `delete_at` is shorthand for `at_mut(i).delete()`: same hole, same
    // untouched len.
    let mut vec: Vector<Value<u32>> = Keypath::default().into();
    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);
    vec.push_empty_slot().set(&30);

    vec.delete_at(1);

    assert_eq!(vec.len(), 3);
    assert_eq!(vec.at(0).try_get().none_if_missing(), Some(10));
    assert_eq!(vec.at(1).try_get().none_if_missing(), None);
    assert_eq!(vec.at(2).try_get().none_if_missing(), Some(30));
});

#[test]
fn test_vector_delete_at_is_bounds_checked() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let addr = <thruput_sdk::ContractAddress as thruput_sdk::Named>::named(
        "test_vector_delete_at_is_bounds_checked",
    );
    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let mut vec: Vector<Value<u32>> = Keypath::default().into();
            vec.push_empty_slot().set(&10);
            vec.delete_at(5);
        })
    });
    assert!(!result.is_success());
}
