//! Test "storage" datatypes: keypaths, handles, vectors and maps

use sdk::contract_test;
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
    let data_keypath = Keypath::root();
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
    let mut map: Map<u32, Value<i32>> = Keypath::root().into();

    let slot: u32 = 3;

    // Check that the entry starts empty
    {
        let v = map.at(&slot).try_get();
        assert_eq!(v, None);
    }

    let mut value: i32 = -14;

    // Set the entry via at_mut().set()
    {
        map.at_mut(&slot).set(&value);
    }

    // Check that the entry has the value
    {
        let v = map.at(&slot).try_get();
        assert_eq!(v, Some(value));
    }

    value += 1;

    // Set via at_mut().set() again
    {
        map.at_mut(&slot).set(&value);
    }

    // Check
    {
        let v = map.at(&slot).try_get();
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
        let v = map.at(&slot).try_get();
        assert_eq!(v, Some(value));
    }
});

contract_test!(test_storable {
    let data_keypath = Keypath::root();
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
    let mut vec: Vector<Value<u32>> = Keypath::root().into();

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
    let mut vec: Vector<Value<u32>> = Keypath::root().into();

    vec.push_with(|slot| slot.set(&10));
    vec.push_with(|slot| slot.set(&20));

    assert_eq!(vec.len(), 2);
    assert_eq!(vec.at(0).get(), 10);
    assert_eq!(vec.at(1).get(), 20);
});

contract_test!(test_vector_push_with_nested_node {
    let mut vec: Vector<Node<InnerValuePair>> = Keypath::root().into();

    vec.push_with(|elem| {
        elem.first.set(&7);
        elem.second.set(&11);
    });

    assert_eq!(vec.len(), 1);
    assert_eq!(vec.at(0).first.get(), 7);
    assert_eq!(vec.at(0).second.get(), 11);
});

contract_test!(test_map_at_nested {
    let mut map: Map<u32, Node<InnerValuePair>> = Keypath::root().into();
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
    let mut val: Value<i32> = Keypath::root().into();

    // Initially empty
    assert_eq!(val.try_get(), None);

    // Set a value
    val.set(&42);
    assert_eq!(val.get(), 42);

    // Overwrite
    val.set(&99);
    assert_eq!(val.get(), 99);

    // Delete
    val.delete();
    assert_eq!(val.try_get(), None);
});

contract_test!(test_value_load_mut {
    let mut val: Value<i32> = Keypath::root().into();

    // Set initial value
    val.set(&10);

    // Modify via load_mut — changes flush on drop
    {
        let mut data = val.load_mut();
        *data = 42;
    }

    assert_eq!(val.get(), 42);
});

contract_test!(test_map_at_value {
    let mut map: Map<u32, Value<i32>> = Keypath::root().into();

    // Write via at_mut
    map.at_mut(&1u32).set(&100);

    // Read via at
    let v = map.at(&1u32).try_get();
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
    let data_keypath = Keypath::root();
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

contract_test!(test_vector_pop {
    let mut vec: Vector<Value<u32>> = Keypath::root().into();

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
    let mut vec: Vector<Value<u32>> = Keypath::root().into();

    vec.push_empty_slot().set(&1);
    vec.push_empty_slot().set(&2);
    vec.push_empty_slot().set(&3);

    vec.delete();
    assert_eq!(0, vec.len());
});

contract_test!(test_vector_delete_verifies_storage_cleared {
    let root = Keypath::root();
    let mut vec: Vector<Value<u32>> = root.clone().into();

    vec.push_empty_slot().set(&100);
    vec.push_empty_slot().set(&200);
    vec.push_empty_slot().set(&300);

    // Verify values are there via manually-constructed keypaths.
    // Vector layout:
    //   len       = root.child_index(0)
    //   values map = root.child_index(1)
    //   element i  = values_kp.child_item(&i)
    let values_kp = root.child_index(1);

    let elem = |kp: &Keypath, idx: u32| -> Value<u32> {
        kp.child_item(&idx).into()
    };

    // Before delete: elements are readable
    assert_eq!(elem(&values_kp, 0).try_get(), Some(100));
    assert_eq!(elem(&values_kp, 1).try_get(), Some(200));
    assert_eq!(elem(&values_kp, 2).try_get(), Some(300));

    // Delete the whole vector
    vec.delete();

    // After delete: all element storage is gone
    assert_eq!(elem(&values_kp, 0).try_get(), None);
    assert_eq!(elem(&values_kp, 1).try_get(), None);
    assert_eq!(elem(&values_kp, 2).try_get(), None);

    // Length storage is also gone
    let len_kp = root.child_index(0);
    let len_val: Value<u32> = len_kp.into();
    assert_eq!(len_val.try_get(), None);
});

contract_test!(test_generic_storage_struct {
    let keypath = Keypath::root();
    let pair: Pair<Value<u32>, Value<u64>> = keypath.into();

    let mut first = pair.first;
    let mut second = pair.second;

    first.set(&42);
    second.set(&100);

    assert_eq!(first.get(), 42);
    assert_eq!(second.get(), 100);
});

contract_test!(test_generic_storage_struct_delete {
    let keypath = Keypath::root();
    let mut pair: Pair<Value<u32>, Value<u64>> = keypath.into();

    pair.first.set(&42);
    pair.second.set(&100);

    pair.delete();

    assert_eq!(pair.first.try_get(), None);
    assert_eq!(pair.second.try_get(), None);
});

contract_test!(test_nnint_delete {
    let keypath = Keypath::root();
    let mut n: NNInt = keypath.into();
    n.set(42);
    assert_eq!(n.get(), 42);
    n.delete();
    assert_eq!(n.try_get(), None);
});

contract_test!(test_vector_pop_nnint {
    let keypath = Keypath::root();
    let mut vec: Vector<NNInt> = keypath.into();

    vec.push_empty_slot().set(10);
    vec.push_empty_slot().set(20);
    assert_eq!(2, vec.len());

    vec.pop();
    assert_eq!(1, vec.len());
    assert_eq!(vec.at(0).get(), 10);
});

contract_test!(test_vector_iter {
    let mut vec: Vector<Value<u32>> = Keypath::root().into();

    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);
    vec.push_empty_slot().set(&30);

    let values: Vec<u32> = vec.iter().map(|v| v.get()).collect();
    assert_eq!(values, [10, 20, 30]);
});

contract_test!(test_vector_iter_mut {
    let mut vec: Vector<NNInt> = Keypath::root().into();

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
    let mut vec: Vector<Value<u32>> = Keypath::root().into();

    vec.push_empty_slot().set(&10);
    vec.push_empty_slot().set(&20);
    vec.push_empty_slot().set(&30);

    let values: Vec<u32> = vec.iter().rev().map(|v| v.get()).collect();
    assert_eq!(values, [30, 20, 10]);
});

contract_test!(test_vector_into_iter {
    let mut vec: Vector<Value<u32>> = Keypath::root().into();

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
    let vec: Vector<Value<u32>> = Keypath::root().into();
    assert_eq!(vec.iter().count(), 0);
});

#[test]
fn test_value_oversized_read_aborts() {
    // Write a raw bytestring larger than ENCODE_MAX (~1 KiB) directly at
    // the storage key used by a Value<u32>.  The typed try_get must abort
    // with StorageSerialization because bytestring_get_encode_buf returns
    // None when the stored length exceeds EncodeBuf's capacity.
    let env = sdk::TestEnvironment::no_cost();
    let addr = <sdk::ContractAddress as sdk::Named>::named(
        "test_value_oversized_read_aborts",
    );
    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let keypath = Keypath::root();
            let key = keypath.generate_key();
            // Write 1100 bytes — well above the ~1 KiB encode-buffer cap.
            sdk::bytestring_set(&key, &[0u8; 1100]);
            let val: Value<u32> = keypath.into();
            val.try_get();
        })
    });
    assert!(!result.is_success());
}

contract_test!(test_map_at_disjoint_mut {
    let mut map: Map<u32, NNInt> = Keypath::root().into();

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
            let mut map: Map<u32, NNInt> = Keypath::root().into();
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
    use sdk::storage::StorageDomain;
    assert_eq!(<StorageDomainPlain as StorageDomain>::DOMAIN, "StorageDomainPlain");
    assert_eq!(<StorageDomainRenamed as StorageDomain>::DOMAIN, "custom");
}

#[test]
fn test_data_fully_consumed() {
    let env = sdk::TestEnvironment::no_cost();
    let addr = <sdk::ContractAddress as sdk::Named>::named(
        "test_data_fully_consumed",
    );

    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let keypath = Keypath::root();
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
                // so try_get must abort; the outer assert checks failure.
                let value: Value<i32> = keypath.clone().into();
                value.try_get();
            }
        })
    });
    assert!(!result.is_success());
}

#[derive(sdk::Storage)]
#[allow(dead_code)]
struct Pinned {
    a: Value<u64>,            // positional 0
    #[slot(7)] b: Value<u64>, // pinned 7
}

#[test]
fn slot_override_remaps_field_keypath() {
    let root = Keypath::root();
    let p = Pinned::from(root.clone());
    // b lives at child_index(7), provably distinct from child_index(1).
    let expect_b = Value::<u64>::from(root.child_index(7));
    let other = Value::<u64>::from(root.child_index(1));
    assert!(p.b.keypath().addresses_same_slot(expect_b.keypath()));
    assert!(!p.b.keypath().addresses_same_slot(other.keypath()));
}
