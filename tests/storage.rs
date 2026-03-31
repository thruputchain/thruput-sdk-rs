//! Test "storage" datatypes: keypaths, handles, vectors and maps

use sdk::Storage;
use sdk::contract_test;
use sdk::storage::*;

#[derive(Debug, xdrs::WriteXDR, Storage)]
struct Data {
    num: i32,
    nums: Vector<u64>,
    nnints: Vector<NNInt>,
    bytes: Bytestring,
    map: Map<u32, NNInt>,
}

contract_test!(test_map {
    let data_keypath = Keypath::new();
    {
        let data: Data = FromKeypath::from_keypath(&data_keypath);
        data.store_keypath(&data_keypath);
    }

    let slot: u32 = 3;
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();

        // Use new at_mut() API
        data.map.at_mut(&slot).strict_inc(7);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
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
    let data: Data = FromKeypath::from_keypath(&data_keypath);
    data.store_keypath(&data_keypath);

    let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
    for i in 0..3 {
        let n = i;
        data.nums.push(n);
    }
    data.store_keypath(&data_keypath);

    let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
    for i in 0..3 {
        let n = data.nums.get(i);
        assert_eq!(n, i);
    }
});

contract_test!(test_vector {
    let mut vec: Vector<u32> = Vector::new_from(Keypath::root(), 0);

    assert_eq!(0, vec.len());
    for i in 0..3 {
        vec.push(i);
    }

    assert_eq!(3, vec.len());
    for i in 0..3 {
        assert_eq!(i as u32, vec.get(i));
    }
});

contract_test!(test_map_at {
    let data_keypath = Keypath::new();
    {
        let data: Data = FromKeypath::from_keypath(&data_keypath);
        data.store_keypath(&data_keypath);
    }

    let slot: u32 = 3;
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        data.map.at_mut(&slot).strict_inc(7);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let v = data.map.at(&slot).get();
        assert_eq!(v, 7);
    }
});

contract_test!(test_map_at_nested {
    let data_keypath = Keypath::new();
    {
        let data: Data = FromKeypath::from_keypath(&data_keypath);
        data.store_keypath(&data_keypath);
    }

    let slot: u32 = 5;
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        data.map.at_mut(&slot).strict_inc(42);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
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
    {
        let data: Data = FromKeypath::from_keypath(&data_keypath);
        data.store_keypath(&data_keypath);
    }

    let slot: u32 = 7;

    // Use at_mut_exclusive — exclusive flag reaches NNInt
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        data.map.at_mut_exclusive(&slot).strict_inc(10);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let v = data.map.at(&slot).get();
        assert_eq!(v, 10);
    }
});
