use groundhog_xdr::storage_item::StorageDeltaFlags;
use sdk::contract_test;
use sdk::get_storage_write_flags;
use sdk::storage::{Balance, Keypath, NNInt};
use sdk::sys_exn::{delete, nnint_internal};
use sdk::types::StorageKey;

contract_test!(raw_writes_replace_flags_including_deletion {
    let key = StorageKey::default();
    assert_eq!(get_storage_write_flags(&key), StorageDeltaFlags::None);
    delete::delete_exclusive(&key);
    assert_eq!(get_storage_write_flags(&key), StorageDeltaFlags::ExclusiveLock);
    nnint_internal::set_add(&key, Some(10), 0, StorageDeltaFlags::None);
    assert_eq!(get_storage_write_flags(&key), StorageDeltaFlags::None);
});

contract_test!(nnint_reads_flags_at_write_time_and_default_still_replaces {
    let path = Keypath::default();
    let key = path.generate_key();
    let mut preserving: NNInt = path.clone().into();
    preserving.preserve_exclusive();
    // Acquired after the handle was made; a cached flag would miss it.
    delete::delete_exclusive(&key);
    preserving.try_strict_inc(10).unwrap();
    assert_eq!(get_storage_write_flags(&key), StorageDeltaFlags::ExclusiveLock);
    let mut ordinary: NNInt = path.into();
    ordinary.inc(1);
    assert_eq!(get_storage_write_flags(&key), StorageDeltaFlags::None);
});

contract_test!(balance_preserves_a_deleted_entry_lock_on_credit {
    let path = Keypath::default();
    let key = path.generate_key();
    let mut balance: Balance = path.into();
    balance.preserve_exclusive();
    delete::delete_exclusive(&key);
    balance.try_lazy_increase(10).unwrap();
    assert_eq!(get_storage_write_flags(&key), StorageDeltaFlags::ExclusiveLock);
    assert_eq!(balance.get(), 10);
});
