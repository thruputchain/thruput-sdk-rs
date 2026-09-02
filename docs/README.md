# What is in `docs/`

Historical engineering notes, not user documentation. Nothing here is
maintained as a guide, and anything in it may describe a design that was
later changed or abandoned.

- **`docs/superpowers/`** — design specs (`specs/`) and implementation plans
  (`plans/`) written while a change was being worked out. They are dated and
  frozen: they record what was intended at that date, including options that
  were rejected and steps that were never taken.
- **`docs/sdk-review-*`** — review logs from code-review passes over the SDK,
  kept as a record of what was found and how each finding was judged. Some
  findings were fixed afterwards, some were deliberately left; the file does
  not track which.

**User documentation lives in `crates/sdk/doc/`** — `lib.md` (the SDK front
page), `macro-contract.md`, `macro-storage.md` and the rest are included into
the published rustdoc, so they are the pages an SDK user reads. Build them
with `./scripts/dev.sh open-docs`. The top-level `README.md` covers building
and developing this repository itself.

Read the files here for background on why something is shaped the way it is,
and check the code before believing any statement about how it works today.
