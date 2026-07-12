# xdrs handoff: `InfallibleWriter<F>` + MAX consts

Goal: give size-critical encoders (sdk abort path now; user-exception path
later) an infallible runtime encode surface, so the CXS width rules stop
being copied into sdk_rs. Measured motivation: encoding through the fallible
`IntoBufWriter`/`encode_into` machinery left ~1.1 KB of avoidable .text in
every contract (fallibility plumbing per field + the failure-handling
apparatus a fallible abort encode forces); an infallible hand encoder
measured −396 B on counter (sdk PR #279, not merged — superseded by this).

## Scope (deliberately small — the omnibus version of this died once)

1. **`trait InfallibleWriter<F>`** — mirrors `Writer<F>`'s vocabulary
   (integers, bool, floats, opaque fixed/variable), `()` returns.
   Contract: panics on buffer overflow; callers guarantee capacity up
   front (that's the point — no `WriteResult` plumbing in encoder code).
   One derive-visible body can then serve both formats, like `WriteXDR`.

2. **One slice-backed impl per format** — essentially `IntoBufWriter`
   minus the `Result`s: `(&mut [MaybeUninit<u8>], written)`. Needs a
   `written()` accessor with the invariant "bytes `0..written()` are
   initialized" (payloads are variable-length; the caller slices its own
   buffer afterward). Document the discipline: ONE writer impl per format
   — each extra impl re-monomorphizes every generic encoder body.

3. **`#[inline(never)]` on the impl's integer methods** (all cascade
   entry points: u64/u32/u16, i64/i32/i16 — or invert to widening
   delegation and attribute only u64/i64). Measured: without it, LLVM
   duplicates the width cascade into every match arm of a big-enum
   encoder while also keeping the outlined copy (+176 B on counter).

4. **Worst-case size consts**: `MAX_CXS_LEN`/`MAX_TXS_LEN: Option<usize>`
   per type (trait or derive-emitted). Fold rule — "boundedness does not
   cross pointers": fold only through by-value layout ([T; N], heapless,
   byte-shaped BytesM/StringM/VecM<u8>); literal `None` through &T,
   &mut T, Box, and element-generic heap containers (VecM<T, N>).
   This is load-bearing: recursive types (contract-spec's
   ContractTypeView, via Option(&Self)/&[Self]) otherwise hit
   E0391 const-eval cycles on recompile — Rust recursion always passes
   through indirection, so cutting folds at pointers makes cycles
   impossible with zero annotations. Do NOT fake u32::MAX bounds for
   unbounded types (poisons every containing type's max).

Non-goals for this pass: no derive-emitted trait-side encode, no changes
to ConstWriter (spec baking keeps using it — trait methods can't be
`const fn` on stable, and don't need to be), no protocol-rs changes.

## Testing

Byte-equality against the existing fallible path (`encode_into`) across
width boundaries: 62/63, ±31/32, 127/128, 255/256, 65535/65536,
u32::MAX±1, i32::MIN/MAX, u64::MAX (+ 128-bit edges), both formats.
This caught real divergences repeatedly during the sdk work.

## sdk_rs follow-up (after publish; additive 0.7.x if possible)

Rework the abort encoder (branch geoff/library-abort-encoder has the
hand-written match + exhaustive wire-compat oracle tests + rv32
measurement harness, all reusable): match calls trait methods, local
width-rule copies deleted, `ABORT_MAX` from the MAX consts. Acceptance:
wire-compat suite green + counter .text ≤ 5612 B
(scripts/metrics/sdk_codesizes_rv32.sh; baseline main 6008).

Later, separately, if user-exception contracts show up in size metrics:
derive-emitted `write<F, W: InfallibleWriter<F>>` bodies (one per type,
format-generic) to fix `throw<E>` monomorphization — that's its own
project with its own measurement gate.
