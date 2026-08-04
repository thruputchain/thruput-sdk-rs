//! Test "storage" datatypes: keypaths, handles, vectors and maps

use sdk::contract_test;
use sdk::exn::NoneIfMissing;
use sdk::storage::*;
use sdk::{Deletable, Storage};

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
    let mut data: Data = data_keypath.clone().into();
    for i in 0..3u64 {
        data.nums.push_with(|slot| slot.set(&i));
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

// (The protocol caps bytestring writes at kBytestringMaxLen = 1024, below
// the SDK's read scratch, so a stored value too large for the read buffer
// can't actually be constructed here — the undecodable-value tests below
// cover the StorageSerialization surface instead.)

#[test]
fn test_value_undecodable_read_reports_err() {
    // Write a raw bytestring that is not a valid encoding of u32 directly
    // at the storage key used by a Value<u32>. try_get reports
    // StorageSerialization as an Err — not a storage-missing error.
    let env = sdk::TestEnvironment::no_cost();
    let addr = <sdk::ContractAddress as sdk::Named>::named(
        "test_value_undecodable_read_reports_err",
    );
    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let keypath = Keypath::default();
            let key = keypath.generate_key();
            sdk::bytestring_set(&key, &[0u8; 900]);
            let val: Value<u32> = keypath.into();
            let err = val.try_get().unwrap_err();
            assert!(!err.is_storage_missing());
            assert_eq!(err, sdk::exn::LibraryExn::StorageSerialization);
        })
    });
    assert!(result.is_success());
}

#[test]
fn test_value_undecodable_get_aborts() {
    // The plain get() on the same undecodable slot aborts with exactly the
    // exception try_get reports.
    let env = sdk::TestEnvironment::no_cost();
    let addr = <sdk::ContractAddress as sdk::Named>::named(
        "test_value_undecodable_get_aborts",
    );
    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let keypath = Keypath::default();
            let key = keypath.generate_key();
            sdk::bytestring_set(&key, &[0u8; 900]);
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
    let env = sdk::TestEnvironment::no_cost();
    let addr = <sdk::ContractAddress as sdk::Named>::named(
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

#[derive(sdk::Storage)]
struct StorageDomainPlain {
    _v: sdk::storage::Value<u64>,
}

#[sdk::storage(domain = "custom")]
struct StorageDomainRenamed {
    _v: sdk::storage::Value<u64>,
}

#[test]
fn storage_domain_defaults_to_ident_and_overrides() {
    // Behavioral: the domain seeds each root field path as its first child
    // frame. Default is the bare ident; #[storage(domain = "...")] overrides.
    let plain = contract_state_root::<StorageDomainPlain>();
    assert!(plain._v.keypath().addresses_same_slot(
        &Keypath::default().child("StorageDomainPlain").child(&0u32)
    ));
    let renamed = contract_state_root::<StorageDomainRenamed>();
    assert!(renamed._v.keypath().addresses_same_slot(
        &Keypath::default().child("custom").child(&0u32)
    ));
}

#[test]
fn test_data_fully_consumed() {
    let env = sdk::TestEnvironment::no_cost();
    let addr = <sdk::ContractAddress as sdk::Named>::named(
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
                    Err(sdk::exn::LibraryExn::StorageSerialization)
                );
            }
        })
    });
    assert!(result.is_success());
}

#[derive(sdk::Storage)]
#[allow(dead_code)]
struct Pinned {
    a: Value<u64>, // positional 0
    #[slot(7)]
    b: Value<u64>, // pinned 7
}

#[test]
fn slot_override_remaps_field_keypath() {
    let root = Keypath::default();
    let p = Pinned::from(root.clone());
    // b lives at child(&7u32), provably distinct from child(&1u32).
    let expect_b = Value::<u64>::from(root.child(&7u32));
    let other = Value::<u64>::from(root.child(&1u32));
    assert!(p.b.keypath().addresses_same_slot(expect_b.keypath()));
    assert!(!p.b.keypath().addresses_same_slot(other.keypath()));
}

#[derive(sdk::Storage)]
#[allow(dead_code)]
struct PositionalAbsoluteIndex {
    #[slot(5)]
    a: Value<u64>, // pinned 5
    b: Value<u64>, // positional: declaration index 1
    c: Value<u64>, // positional: declaration index 2
}

#[test]
fn positional_fields_use_absolute_declaration_index() {
    let root = Keypath::default();
    let s = PositionalAbsoluteIndex::from(root.clone());
    // c is the 3rd field -> child(&2u32), NOT child(&1u32).
    assert!(
        s.c.keypath().addresses_same_slot(
            Value::<u64>::from(root.child(&2u32)).keypath()
        )
    );
    assert!(
        !s.c.keypath().addresses_same_slot(
            Value::<u64>::from(root.child(&1u32)).keypath()
        )
    );
    // b -> child(&1u32); a -> child(&5u32).
    assert!(
        s.b.keypath().addresses_same_slot(
            Value::<u64>::from(root.child(&1u32)).keypath()
        )
    );
    assert!(
        s.a.keypath().addresses_same_slot(
            Value::<u64>::from(root.child(&5u32)).keypath()
        )
    );
}
