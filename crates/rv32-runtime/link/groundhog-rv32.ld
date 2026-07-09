/* Groundhog RV32 contract linker script.
 *
 * The hogd VM is modified-Harvard: PC indexes an immutable CODE memory; loads
 * and stores address a separate DATA memory. The two are INDEPENDENT address
 * spaces in the VM, so we lay them out as two MEMORY regions whose virtual
 * addresses deliberately overlap numerically — they can never collide because
 * a value used as a jump target is fetched from CODE while a value used by a
 * load/store hits DATA. This is what lets DATA start near address 0 instead of
 * being stacked above .text: the stored initial-memory image is then compact
 * (just the real .rodata/.data), with none of the code-sized leading-zero
 * "shadow" the old single-region layout produced.
 *
 * Because CODE and DATA VMAs overlap, the linker must be invoked with
 * `--no-check-sections` (set in the contract .cargo/config.toml); the overlap
 * is intentional, not a layout bug.
 *
 * DATA is laid out stack-first (the wasm-ld `--stack-first` discipline):
 *
 *   0x0 .. __stack_top    stack, growing DOWN from __stack_top
 *   __stack_top ..        .rodata / .data / .bss
 *   __heap_base ..        heap, growing UP toward __memory_size
 *
 * With the stack at the bottom, the regions that grow (stack down, heap up)
 * both grow toward a hard boundary instead of toward each other: a stack
 * overflow wraps sp below zero to ~0xFFFFFFxx, far outside the guest
 * memory the VM provisions — an immediate OOB trap — rather than silently
 * corrupting heap or statics as the previous stack-at-top layout allowed.
 * The heap gets a hard bound at the true guest memory size (__heap_end =
 * __memory_size), reclaiming the old top-of-memory stack reserve. Note
 * that __memory_size is intentionally smaller than the DATA region: the
 * region lengths are relaxed sanity bounds, not the real limits.
 *
 * The stored image stays compact: artifact format v5 records
 * `initial_memory_start` (the guest VA where the image is loaded — the
 * lowerer sets it to the lowest data-section VA), so the stack region
 * below the statics is never stored, only zero-filled at instantiation.
 *
 * Layout invariants enforced by the hogd lowerer (see
 * docs/rv32/rv32-vm-experimental-design.md, "Rust Fixture And Lowering"):
 *
 *   - .text starts at virtual address 0 (so PC space and VA space match).
 *   - No data section may touch the lowerer's null guard [0, 0x8) — the
 *     stack reserve covers those addresses, so sections start well above.
 *   - No executable sections other than .text.
 *   - No GOT / PLT / dynamic relocations.
 *   - No init_array / eh_frame.
 *
 * The stack reserve starts at address 0. In principle a call chain that
 * fills the stack to the exact byte would place a local at address 0
 * (whose reference would be null); we accept that edge — such a frame is
 * one push away from the wrap trap anyway. The old rationale for starting
 * DATA at 0x8 (no `static` at address 0) no longer applies: statics now
 * sit above the stack reserve.
 */

ENTRY(_start)

/* Stack reserve size. 24K by default: no-alloc contracts keep their
 * ScratchBufs (4K each, several live at once on cross-call paths) on the
 * stack, so the reserve must hold a few of them plus call frames. The
 * reserve costs nothing in the stored artifact — the lowerer trims it via
 * `initial_memory_start` — only address space shared with the heap, which
 * no-alloc contracts don't use at all.
 *
 * Per-contract override (must be a multiple of 16, enforced by the ASSERT
 * at the bottom): pass a linker --defsym in the contract's rustflags,
 * e.g. in .cargo/config.toml under [target.riscv32im-unknown-none-elf]:
 *
 *   "-C", "link-arg=--defsym=__stack_size=0x8000",
 *
 * PROVIDE yields to such an external definition.
 */
PROVIDE(__stack_size = 24K);

/* True guest data-memory size. The VM provisions exactly this much
 * (memory_bytes in the artifact header). MUST stay in lockstep with
 * MAX_MEMORY_BYTES in groundhog-rpc crates/rv32/artifact/src/format.rs
 * and hogd/hogexec/runtime/rv32/artifact.h. */
__memory_size = 64K;

/* Relaxed sanity bounds only — deliberately never-binding. The real
 * 64 KiB code/memory limits are enforced by the CLI lowerer / artifact
 * writer, which produces proper error messages instead of a cryptic
 * "section '.text' will not fit in region 'CODE'". Keep these <= 1M so
 * all direct `jal` calls stay in range and relocation errors remain
 * impossible. */
MEMORY
{
  CODE (rx) : ORIGIN = 0, LENGTH = 1M
  DATA (rw) : ORIGIN = 0, LENGTH = 1M
}

/* Stack reserve at the bottom of DATA. _start loads sp from __stack_top
 * (16-byte aligned) and the stack grows down toward 0. No output section
 * is emitted for the reserve — data sections are simply placed above it —
 * so the lowerer's section allowlist is unaffected.
 */
__stack_bottom = ORIGIN(DATA);
__stack_top = __stack_bottom + __stack_size;

SECTIONS
{
  .text 0 : ALIGN(4)
  {
    /* _start is the linker entry and the sole dispatch surface: the VM
     * always enters here at PC 0 with the method id in a0/a1. KEEP it so
     * dead-code elimination can't drop it.
     */
    KEEP(*(.text._start))

    *(.text .text.*)
  } > CODE

  .rodata __stack_top : ALIGN(8)
  {
    /* Guest dispatch table: one `Entrypoint` record per #[contract] method,
     * emitted by the groundhog-macros expansion via
     * #[link_section = "groundhog_rv32_entrypoints"]. Placed inside .rodata
     * with explicit bracket symbols so the table rides the lowerer's
     * existing .rodata -> initial-memory copy, instead of an orphan section
     * the PROGBITS allowlist would skip. Section and symbol names are owned
     * by this repo end-to-end (macro emits, this script brackets,
     * rv32-runtime/src/start.rs reads) — deliberately NOT the linker's
     * __start_<section> auto-encapsulation convention, so this script is
     * unambiguously the single definer. */
    . = ALIGN(8);
    __groundhog_rv32_entrypoints_start = .;
    KEEP(*(groundhog_rv32_entrypoints))
    __groundhog_rv32_entrypoints_end = .;

    *(.srodata .srodata.*)
    *(.rodata .rodata.*)
  } > DATA

  . = ALIGN(8);
  .data : ALIGN(8)
  {
    *(.sdata .sdata.*)
    *(.data .data.*)
  } > DATA

  . = ALIGN(8);
  .bss (NOLOAD) : ALIGN(8)
  {
    __bss_start = .;
    *(.sbss .sbss.*)
    *(.bss .bss.*)
    *(COMMON)
    __bss_end = .;
  } > DATA

  /* Heap grows from __heap_base toward __memory_size, the true guest
   * memory size. The allocator checks __heap_end as the bound; with the
   * stack parked at the bottom of DATA nothing lives above the heap, so
   * the bound is the end of guest memory. Deliberately NOT derived from
   * LENGTH(DATA): the DATA region is a relaxed sanity bound, and a heap
   * bound above the real memory size would turn heap exhaustion from a
   * clean alloc failure into a VM OOB trap.
   */
  . = ALIGN(8);
  __heap_base = .;
  __heap_end = __memory_size;

  /DISCARD/ :
  {
    *(.got .got.*)
    *(.got.plt .got.plt.*)
    *(.plt .plt.*)
    *(.data.rel.ro .data.rel.ro.*)
    *(.tdata .tdata.*)
    *(.tbss .tbss.*)
    *(.init_array .init_array.*)
    *(.eh_frame .eh_frame.*)
    *(.debug_* .debug*)
    *(.comment)
  }
}

ASSERT(__heap_base <= __heap_end, "rv32 data sections leave no room for heap");
/* _start loads sp from __stack_top with no rounding; the RISC-V ABI
 * requires sp to be 16-byte aligned. */
ASSERT(__stack_top % 16 == 0, "__stack_top must be 16-byte aligned");
/* Fail a bogus __stack_size override here with a clear message. This is
 * DX, not a safety boundary: the lowerer already rejects any data section
 * inside its [0, 8) null guard, so statics at address 0 can never reach a
 * deployable artifact. */
ASSERT(__stack_size >= 16, "__stack_size must be at least one 16-byte slot");
