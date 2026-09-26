# Storage flags: SetStorageFlags, GetStorageFlags and ClearStorageFlags

An alternative to [hogd #4156](https://github.com/groundhogchain/hogd/pull/4156),
written against the allowance design in
[sdk_rs #482](https://github.com/groundhogchain/sdk_rs/pull/482). The protocol
side is [protocol #190](https://github.com/groundhogchain/protocol/pull/190).

## The problem

#482 promises locks that outlive the write that takes them.
`adjust_allowance(spender, difference, exclusive=true)` conflicts with other
writers to the pair, and `clear_allowance` and `clear_balance` delete under an
exclusive lock. Today a later write to the same key in the same transaction (a
nested `transfer_from`, a regrant, a credit) replaces the key's flags with its
own, so the transaction commits a shared change and a concurrent writer merges
with it ([hogd #3006](https://github.com/groundhogchain/hogd/issues/3006)).

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

### ABI version

The three writes change layout, and a guest built against the old layout
passes flags they no longer read. Its locks would disappear without an error,
so hogd moves `kArtifactSyscallAbiVersion` to 2 and refuses to load v1
artifacts.

The version is stamped by the lowerer (`rv32-lower`, `groundhog` CLI), not by
the SDK. A lowerer that stamps v2 will also stamp it on an ELF built with an old
SDK. The SDK should record its syscall ABI in the ELF (a section like
`groundhog_rv32_entrypoints`) and the lowerer should copy it into the header,
or refuse an ELF without it. Neither is part of this change.

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
- The sequence lives in one place, `sys_result::flags::request_after_write`,
  which every SDK write goes through. The setter cannot fail right after a
  successful write for a known flag; if it does, the SDK throws, which reverts
  the frame and its write instead of leaving the write without its lock.
- Leaf handles (`Value`, `NNInt`, `Balance`, `Bytestring`) gain `flags()`,
  `set_flags()` (OR) and `clear_flags()`. `StorageFlags` is an alias of
  `StorageDeltaFlags`. The free functions are `storage_flags`,
  `set_storage_flags` and `clear_storage_flags` (throwing, re-exported at the
  crate root) and their `sys_result::flags` counterparts.
- Ordinary writes add no syscall. Exclusive writes add one.
- The private policy helpers are renamed: `Keypath::flags` and `LeafKey::flags`
  are now `write_flags`, since `flags` on a handle means the host's flags.

## Allowances in #482

The SDK-based tokens need no code changes: a nested `transfer_from` after an
exclusive `adjust_allowance`, a regrant after `clear_allowance` and a credit
after `clear_balance` all keep the lock. The ERC20 interface docs now say how
long the lock lasts.

`erc20_low_level` and `erc20_raw_storage` added a lock by re-reading the pair
and rewriting it with `ExclusiveLock`. They now call `set_storage_flags` after
the increment.

`erc20/tests/allowances.rs` adds
`exclusive_adjustment_holds_through_a_nested_spend_in_both_orders` and
`cleared_balance_stays_locked_through_a_credit_in_both_orders`: in each, the
locked transaction ends on an ordinary write that would merge with the
concurrent one under last-write-wins flags.

## Compared with the other approaches

| | #4156 (`flags_ \|= flags`) | `prior_flags` outputs | This design |
| --- | --- | --- | --- |
| Lock lifetime | Every flag sticks for the transaction | Per write; the SDK reissues | Until `ClearStorageFlags` |
| Releasing a lock deliberately | Not possible | Rewrite without the flag | `ClearStorageFlags` |
| Contracts not using the SDK | Covered | Must reissue themselves | Covered once they set the flag |
| Extra cost | None | A second write when a lock would drop | One syscall per exclusive write |
| ABI | Unchanged | Outputs added to 15, 17, 18 | Flags removed from 15, 17, 18; syscalls 36 to 38 added |

## Rollout

1. protocol #190.
2. hogd: host semantics, ABI v2 and tests (branch
   `geoph/amazing-edison-nhzhz9`), then a test-ffi release.
3. A `groundhog-xdr` generated from the new `syscalls.yaml`. This branch does
   not build against 0.39.0.
4. sdk_rs: bump `groundhog-xdr` and the test-ffi pin (`groundhog-sys` uses
   0.32.2), then this branch.
5. A lowerer and CLI that stamp syscall ABI v2, and a rebuild of hogd's
   integration-test contracts and RV32 fixtures with the new SDK.

## Deferred

- A lock on a key the transaction has not written (it would need a lock-only
  change in hog_db), and a `NoChange` delta.
- Lock protection in read-only frames.
- `RetrocausalQueueInsert` counts as a write, so `SetStorageFlags` after an
  insert now produces a permanent-exclusive RCQ insert through syscalls. hog_db
  handles it (its direct-delta tests), but the syscall-level tests that
  `rcq_doctored_proposal_data_test.cc` sketches are not written, and RCQ
  handles get no flag accessors here.
- `CapacityConstraint` stages its writes until it flushes, so it gets no
  `set_flags`; its exclusive policy still applies at flush.

## Validation

- protocol: `generate_syscalls_doc.py` validates the new yaml, and
  `scripts/xdrformat -c` passes.
- hogd: not built or run here (no private submodules, RV32 contracts or
  google-benchmark). Every changed translation unit that does not include a
  generated contract header or benchmark header passes `-fsyntax-only` against
  headers generated from the new protocol, including the new tests. clang-tidy
  is clean on `syscall_storage_flags.cc`. The new format fingerprints were
  computed from `artifact.h` and pinned.
- sdk_rs, against a local `groundhog-xdr` carrying the new yaml: clippy is
  clean for the SDK, native-exec and the ERC20 examples, including the riscv32
  guest build of `erc20`, and rustfmt has nothing to change in the touched
  files. The pinned test-ffi 0.32.2
  predates syscalls 36 to 38, so every exclusive write there throws
  `NoSyscall`. The SDK suite passes 343 tests; its 27 failures (24 existing
  storage tests and the 3 new ones) all stop on syscall 36, 37 or 38.
  native-exec passes. Of the 75 example-contract packages, 13 have failures:
  all of them stop on syscall 36, except rv32 tests that need prebuilt
  `.rv32a` artifacts this environment lacks. The new tests need a test-ffi
  built from the new hogd.
