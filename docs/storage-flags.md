# Storage flags: SetStorageFlags, GetStorageFlags and ClearStorageFlags

An alternative to [hogd #4156](https://github.com/groundhogchain/hogd/pull/4156).
The protocol side is [protocol #190](https://github.com/groundhogchain/protocol/pull/190)
and the host side [hogd #4158](https://github.com/groundhogchain/hogd/pull/4158).

## The problem

An exclusive write takes a lock on its key. Before this change, a later write
to the same key in the same transaction replaced the key's flags with its own.
A transaction that locked a key and then wrote it again ordinarily committed a
shared change, and a concurrent writer merged with it
([hogd #3006](https://github.com/groundhogchain/hogd/issues/3006)).

The allowance design in
[sdk_rs #482](https://github.com/groundhogchain/sdk_rs/pull/482) needs locks
that outlive the write that takes them.
`adjust_allowance(spender, difference, exclusive=true)` conflicts with other
writers to the pair, and `clear_allowance` and `clear_balance` delete under an
exclusive lock. A nested `transfer_from`, a regrant or a credit later in the
same transaction would drop that lock.

## Design

A transaction keeps one flags value per storage key, managed separately from
its writes. `ExclusiveLock` is the only flag today.

### Host

| Syscall | Behavior |
| --- | --- |
| `BytestringSet` (15), `NonnegativeIntSetAdd` (17), `DeleteObject` (18) | No flags argument any more. Writes never change a key's flags, RCQ conversions included, and a failed write leaves them alone. |
| `SetStorageFlags(key, flags)` (36) | ORs `flags` in. Fails with `InvalidSyscallInput` for an unknown flag, or if no write to the key is pending in the calling frame's view. |
| `GetStorageFlags(key)` (37) | The visible flags; `None` if no write is pending. Read-only. |
| `ClearStorageFlags(key)` (38) | Removes every flag. Does nothing if no write is pending. |

A write is pending when it was made earlier in this frame, or inherited from a
caller or a successful callee. A pending deletion qualifies; a value that only
exists in the block snapshot does not. A contract cannot re-enter itself, so a
key's view is inherited when the same contract runs more than once in a
transaction, as a token does when a wallet calls it twice.

The flags live in `ItemView::flags_`, so they follow the existing frame
copy-on-write: successful frames pass changes up, and reverted frames and
failed `TryInvoke` calls discard flag changes with their writes. Every push to
the database (a `TryInvoke` return, or the commit) carries the key's
cumulative delta and its flags at that point. Clearing after a `TryInvoke`
push therefore withdraws permanent exclusivity and its storage refund at the
next push, through hog_db's existing `ItemChange::narrow()`; the ordinary
exclusivity rollback needs stays until commit.

Each flag syscall costs `kStorageAccessCost`, the same frame walk a write
pays.

### Compatibility

The syscall ABI version stays at 1, and artifacts built against the old write
layout still load. They pass a flags argument that the three writes no longer
read, so their exclusive writes take no lock until the contract is rebuilt
with this SDK.

### SDK

The exclusive access mode stays a write policy:

```rust
map.at_mut_exclusive(&key).set(&value); // write, then SetStorageFlags
map.at_mut(&key).set(&next_value);      // still exclusive
map.at_mut(&key).clear_flags();         // no longer
```

- Each successful write through an exclusive handle (`at_mut_exclusive`,
  `set_exclusive`, `delete_exclusive`, an exclusive `Keypath`) is followed by
  `SetStorageFlags(ExclusiveLock)`. Building, reading and dropping a handle do
  nothing to the host's flags, and ordinary handles never clear them. Nested
  handles pass the policy to each leaf they write.
- A write's policy is a `bool`, not a flag set, so a write can request
  `ExclusiveLock` and nothing else. Every SDK write goes through
  `sys_exn::flags::lock_after_write`, right after a write that left the key
  pending, so the setter cannot fail there. If it does anyway, the SDK throws,
  which reverts the frame and its write instead of leaving the write without
  its lock.
- Arbitrary flags go only through `set_storage_flags` and
  `Flaggable::set_flags`. Each is one syscall that writes nothing, so an
  unknown flag is rejected with storage unchanged: `Err` from
  `sys_result::flags::set_storage_flags`, a throw from the others. The write
  helpers that took a flag set are gone: `delete_with_flags` is removed, and
  `Deletable::delete_internal` (and so the `Deletable` derive) and
  `sys_exn::nnint_internal::set_add` take `exclusive: bool`.
- Leaf handles (`Value`, `NNInt`, `Balance`, `Bytestring`) implement the
  sealed `Flaggable` trait: `flags()`, `set_flags()` (OR) and
  `clear_flags()`. `StorageFlags` is an alias of
  `StorageDeltaFlags`. The free functions are `storage_flags`,
  `set_storage_flags` and `clear_storage_flags` (throwing, re-exported at the
  crate root) and their `sys_result::flags` counterparts.
- Ordinary writes add no syscall. Exclusive writes add one.
- `Keypath::flags` and `LeafKey::flags` are removed: `is_exclusive()` is the
  write policy, and `flags` on a handle now means the host's flags.

## Allowances in #482

#482 is not part of this change. On top of it, #482's SDK-based tokens keep
their locks with no code changes: a nested `transfer_from` after an exclusive
`adjust_allowance`, a regrant after `clear_allowance` and a credit after
`clear_balance`. Its `erc20_low_level` and `erc20_raw_storage` add a lock by
re-reading the pair and rewriting it with `ExclusiveLock`. They can call
`set_storage_flags` after the increment instead.

## Compared with the other approaches

| | #4156 (`flags_ \|= flags`) | `prior_flags` outputs | This design |
| --- | --- | --- | --- |
| Lock lifetime | Every flag sticks for the transaction | Per write; the SDK reissues | Until `ClearStorageFlags` |
| Releasing a lock deliberately | Not possible | Rewrite without the flag | `ClearStorageFlags` |
| Contracts not using the SDK | Covered | Must reissue themselves | Covered once they set the flag |
| Extra cost | None | A second write when a lock would drop | One syscall per exclusive write |
| Syscalls | Unchanged | Outputs added to 15, 17, 18 | Flags removed from 15, 17, 18; syscalls 36 to 38 added |

## Rollout

1. protocol #190 (merged), released in protocol-rs 0.40.0.
2. hogd #4158 (merged), released as test-ffi 0.33.0.
3. sdk_rs: this branch, on protocol-rs 0.40.0 and test-ffi 0.33.0.
   `syscall_misc_tests` in `integration_test/contracts` still passes flags to
   the raw write syscalls, so it has to drop them before this branch's
   integration build passes.
4. Rebuild hogd's integration-test contracts with the new SDK, so their
   exclusive writes lock again. Until then `airdrop_faucet` and
   `payment_channel` take no lock, and a hogd test for each fails.

## Deferred

- A lock on a key the transaction has not written (it would need a lock-only
  change in hog_db), and a `NoChange` delta.
- Lock protection in read-only frames.
- `RetrocausalQueueInsert` counts as a write, so `SetStorageFlags` after an
  insert now produces a permanent-exclusive RCQ insert through syscalls. hog_db
  handles it (its direct-delta tests), and hogd #4158 tests through syscalls
  that a lock on a bytestring survives the key's conversion to a queue and
  keeps another transaction's insert out. The consume-side proposal and
  validation roundtrips that `rcq_doctored_proposal_data_test.cc` sketches are
  not written, and RCQ handles get no flag accessors here.
- `CapacityConstraint` stages its writes until it flushes, so it gets no
  `set_flags`; its exclusive policy still applies at flush.

## Validation

- protocol: `generate_syscalls_doc.py` validates the new yaml, and
  `scripts/xdrformat -c` passes.
- hogd: the new tests pass in #4158's CI. Two contract tests fail there until
  rollout step 4.
- sdk_rs, on protocol-rs 0.40.0 and test-ffi 0.33.0: the SDK suite passes,
  the three new storage tests included, as do clippy, rustdoc and the riscv32
  build of the SDK.
