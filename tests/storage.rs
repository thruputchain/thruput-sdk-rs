//! Test "storage" datatypes: keypaths, handles, vectors and maps

use sdk::Storage;
use sdk::contract_test;
use sdk::storage::*;

#[derive(Debug, Storage)]
#[allow(dead_code)]
struct Data {
    num: Value<i32>,
    nums: Vector<Value<u64>>,
    nnints: Vector<NNInt>,
    bytes: Bytestring,
    map: Map<u32, NNInt>,
}

contract_test!(test_map {
    let data_keypath = Keypath::new();
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
    // Use Map<u32, Value<i32>> with the new API
    let keypath = Keypath::new();
    let mut map: Map<u32, Value<i32>> = Map::new(&keypath);

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
    let data_keypath = Keypath::new();
    let mut data: Data = data_keypath.clone().into();
    for i in 0..3u64 {
        data.nums.push().set(&i);
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
        vec.push().set(&i);
    }

    assert_eq!(3, vec.len());
    for i in 0..3u32 {
        assert_eq!(i, vec.at(i).get());
    }
});

contract_test!(test_map_at {
    let data_keypath = Keypath::new();
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

contract_test!(test_map_at_nested {
    let data_keypath = Keypath::new();
    let slot: u32 = 5;
    {
        let mut data: Data = data_keypath.clone().into();
        data.map.at_mut(&slot).strict_inc(42);
    }

    {
        let data: Data = data_keypath.into();
        let val = data.map.at(&slot).try_get().unwrap_or(0);
        assert_eq!(val, 42);
    }
});

contract_test!(test_value_basic {
    let keypath = Keypath::new();
    let mut val: Value<i32> = Value::new(&keypath);

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
    let keypath = Keypath::new();
    let mut val: Value<i32> = Value::new(&keypath);

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
    let keypath = Keypath::new();
    let mut map: Map<u32, Value<i32>> = Map::new(&keypath);

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
    let data_keypath = Keypath::new();
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

    vec.push().set(&10);
    vec.push().set(&20);
    vec.push().set(&30);
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

    vec.push().set(&1);
    vec.push().set(&2);
    vec.push().set(&3);

    vec.delete();
    assert_eq!(0, vec.len());
});

contract_test!(test_nnint_delete {
    let keypath = Keypath::new();
    let mut n: NNInt = keypath.into();
    n.set(42);
    assert_eq!(n.get(), 42);
    n.delete();
    assert_eq!(n.try_get(), None);
});

contract_test!(test_vector_pop_nnint {
    let keypath = Keypath::new();
    let mut vec: Vector<NNInt> = keypath.into();

    vec.push().set(10);
    vec.push().set(20);
    assert_eq!(2, vec.len());

    vec.pop();
    assert_eq!(1, vec.len());
    assert_eq!(vec.at(0).get(), 10);
});

#[test]
fn test_data_fully_consumed() {
    let env = sdk::TestEnvironment::no_cost();

    let result = sdk::contract_exec(env, || {
        let keypath = Keypath::new();
        {
            let mut value: Value<(i32, i32)> = Value::new(&keypath.clone());
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
            let value: Value<i32> = keypath.clone().into();
            value.try_get();
            assert!(false, "unreachable");
        }
    });
    assert!(result.is_err());
}
