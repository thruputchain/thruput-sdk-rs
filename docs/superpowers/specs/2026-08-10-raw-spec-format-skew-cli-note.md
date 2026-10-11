# Note: raw spec format skew — the groundhog-rpc side

## The problem this closes

A partner's `thruput build` failed with a confusing parse error. Cause: the
raw spec fragment format changed (PR #321 added `emits` to
`SpecInterfaceFragment`), the SDK crate was bumped, and the CLI was bumped —
but the *pairing* was never checked. Before/before and after/after both work;
**after-SDK with before-CLI** breaks.

It breaks badly because the `groundhogspecraw*` section body is an unframed
concatenation of XDR `SpecFragment` values — no length prefix, no framing, no
version in the payload. An older parser reads the field set it knows, stops
short of `emits`, and resumes decoding *inside* the previous fragment. Every
later fragment decodes as garbage, surfacing as `InvalidRawFragment(<xdrs
error>)`, "parser made no progress", or a nonsense interface name. Nothing in
the stream distinguishes "newer than me" from "corrupt".

The agreed fix is **fail fast, clearly, in both directions**. The version is
carried by the section *name* so skew is detectable before a single fragment
byte is read.

## What sdk_rs now provides (done, on this branch)

`groundhog-contract-spec` owns the format version, since it owns the fragment
types. New module `crates/contract-spec/src/format.rs`, re-exported from both
`groundhog_contract_spec` and `groundhog_spec_tool`:

| Item | Value / signature | Use |
| --- | --- | --- |
| `RAW_SPEC_FORMAT` | `u32 = 1` | what this build reads/writes |
| `RAW_SECTION_PREFIX` | `"groundhogspecraw"` | match on this, **not** the full name |
| `RAW_SECTION_NAME` | `"groundhogspecrawv1"` | what this SDK emits |
| `FINAL_SECTION_NAME` | `"groundhogspecv0"` | unchanged |
| `classify_section(&str)` | `-> SpecSection` | `Raw(u32)` / `Final` / `Other` |
| `check_raw_spec_format(u32)` | `-> Result<(), SpecToolError>` | the shared check |

Two new `SpecToolError` variants, `RawSpecFormatTooNew { found, supported }`
and `RawSpecFormatTooOld { found, supported }`. Their `Display` names both
versions and the fix for that direction — **please surface them verbatim
rather than rewording**, so the two repos say the same thing.

`rewrite_wasm`/`rewrite_wasm_with_spec` now classify by prefix and call
`check_raw_spec_format` *before* decoding, so the wasm path is already covered
for callers using it (`contract_spec.rs:73` is one).

**The section name changed to `groundhogspecrawv1`.** Contracts built with
this SDK no longer carry `groundhogspecrawv0`. Format 0 is deliberately
unsupported in both directions: two incompatible encodings shipped under that
name, so those bytes cannot be interpreted from the name alone.

## Work owned by groundhog-rpc

### 1. Preflight, before compiling — the main UX win

`thruput build` shells out to cargo, so it can detect skew in ~2s instead of
after a full contract compile. `groundhog-contract-spec`'s manifest now carries

```toml
[package.metadata.groundhog]
raw_spec_format = 1
```

and `cargo metadata` surfaces `package.metadata` for registry dependencies
(verified). So: read the contract's resolved `groundhog-contract-spec` package,
pull `metadata.groundhog.raw_spec_format`, compare against the linked
`groundhog_spec_tool::RAW_SPEC_FORMAT` via `check_raw_spec_format`. **No
hand-maintained version table** — the format version travels with the crate.

Details that matter:

- **`cargo_metadata_with_artifacts` passes `--no-deps`** (`build.rs:653`),
  which reports only workspace members. This check needs the resolved graph,
  so it needs a call *without* `--no-deps`. That is slower and may hit the
  network on a cold checkout — worth a separate call rather than widening the
  existing one, since other callers don't need deps.
- **Absent key ⇒ format 0.** A `groundhog-contract-spec` predating the key is
  pre-versioning, which is genuinely unsupported; the `TooOld` message is
  correct there.
- **Absent package ⇒ skip.** Not a Groundhog contract; don't invent an error.
- Model it on `check_rv32_build_config` (`build.rs:688`), which already
  establishes the probe-then-refuse-or-warn-before-compiling idiom.

### 2. rv32 artifact path — currently silent

`load_rv32_contract_spec_from_bytes` (`contract_spec.rs:78`) matches
`section.name != RAW_SECTION_NAME` exactly (line 87). With the rename, a
`v1` section is **skipped**, `final_data` stays `None`, and the function
returns `Ok(None)` — reported as "this contract has no spec", which is the
wrong diagnosis and arguably worse than the parse error. Switch to
`classify_section` + `check_raw_spec_format` before `parse_raw_fragments`.

### 3. Section-name constants in the lowerer

`crates/rv32/lower/src/lower.rs:28` hardcodes
`SPEC_METADATA_SECTIONS = ["groundhogspecrawv0", "groundhogspecv0"]`, which
`is_spec_metadata_section` uses to skip spec sections (`lower.rs:131`).

**This is the first thing that breaks, and it fails loudly.** Confirmed
against sdk_rs PR #333 CI:

```
error: RV32 lowering failed: unexpected alloc section: groundhogspecrawv1
```

`groundhogspecrawv1` misses the strip list, falls through to the
`ALLOC_ALLOWLIST` check at `lower.rs:137`, and hard-errors. That is the
*safe* failure — it fails closed rather than leaking spec content into the VM
image — but it means **every rv32 build fails until this is fixed**, so it is
a blocker rather than a cleanup.

Match on `RAW_SECTION_PREFIX` in `is_spec_metadata_section` rather than adding
`v1` to the list, so the next format bump doesn't reopen it.

### 4. Bidirectional tests

`support::minimal_rv32_elf_with_section` (`crates/tools/cli/tests/build_test.rs:642`)
is the fixture builder. Four cases, mirroring sdk_rs's
`crates/spec-tool/tests/format_skew.rs`:

| Path | Direction | Asserts |
| --- | --- | --- |
| preflight | manifest metadata `RAW_SPEC_FORMAT + 1` | refuses **before** cargo runs |
| preflight | manifest metadata `RAW_SPEC_FORMAT - 1` | refuses **before** cargo runs |
| rv32 artifact | section `…rawv{N+1}` | `RawSpecFormatTooNew`, not "no spec" |
| rv32 artifact | section `…rawv{N-1}` | `RawSpecFormatTooOld`, not "no spec" |

Use a deliberately **invalid** fragment body in the artifact fixtures. The
check must fire before decoding; well-formed bytes would let a too-late check
pass the test.

### 5. Release sequencing

The new `groundhog-spec-tool` has to reach the CLI **before** contracts built
with the new SDK can be built there — otherwise every such contract hits
`TooNew`. Same publish→rpc→CLI→pin-bump chain as the 0.18 release.

## What is deliberately not done

- **No dual-reading.** Per the "fail fast" decision, a new CLI rejects an
  old-format artifact rather than keeping retired fragment types alive. Every
  format bump therefore forces contract rebuilds, not just CLI upgrades.
- **No fix for already-published CLIs.** A CLI released before the preflight
  lands still fails confusingly; those users must upgrade. This was an explicit
  scoping decision, and it belongs in the release notes.

## Guard rails in sdk_rs (context, no action needed)

So the next format change cannot repeat this silently:

- `crates/contract-spec/tests/raw_format_golden.rs` — encodes every
  `SpecFragment` variant against a checked-in hex blob. **This is the test that
  would have caught #321.** Its failure message lists the four mirrors to bump.
- `golden_covers_every_fragment_variant` — the golden can't quietly stop
  covering a variant.
- `manifest_metadata_matches_raw_spec_format` — pins `Cargo.toml`'s key to the
  constant, i.e. pins the thing your preflight reads.
- `raw_section_name_matches_its_parts` — pins the name to the version.
- `sdk_macro_link_section_literals_match_raw_section_name`
  (`crates/spec-tool/src/raw.rs`) — pins the three `link_section` literals in
  `groundhog-macros`. It previously covered only two of the three sites;
  `derive_event.rs` was unpinned and has been added.
