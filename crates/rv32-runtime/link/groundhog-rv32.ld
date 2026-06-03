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
 * Layout invariants enforced by the hogd lowerer (see
 * docs/rv32/rv32-vm-experimental-design.md, "Rust Fixture And Lowering"):
 *
 *   - .text starts at virtual address 0 (so PC space and VA space match).
 *   - .rodata / .data / .bss live in the DATA space at low addresses.
 *   - the heap stops below a reserved stack region at the top of DATA.
 *   - No executable sections other than .text.
 *   - No GOT / PLT / dynamic relocations.
 *   - No init_array / eh_frame.
 *
 * DATA starts at 0x8 rather than 0 so that no `static` is ever placed at
 * address 0: a Rust `&'static` to an object at address 0 is a null reference,
 * which is UB (Rust guarantees references are non-null). 0x8 is one .rodata
 * alignment unit — the minimal guard that keeps address 0 unused. It costs only
 * ~8 leading-zero bytes in the stored image (vs. the old code-sized shadow).
 */

ENTRY(_start)

__stack_size = 4K;

MEMORY
{
  CODE (rx) : ORIGIN = 0,   LENGTH = 64K
  DATA (rw) : ORIGIN = 0x8, LENGTH = 64K - 0x8
}

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

  .rodata : ALIGN(8)
  {
    /* Guest dispatch registry (linkme distributed_slice). Placed inside
     * .rodata with explicit __start_/__stop_ encapsulation symbols so the
     * table rides the lowerer's existing .rodata -> initial-memory copy,
     * instead of an orphan section the PROGBITS allowlist would skip.
     * Section names are produced by linkme from the slice name
     * GROUNDHOG_RV32_ENTRYPOINTS. */
    . = ALIGN(8);
    __start_linkme_GROUNDHOG_RV32_ENTRYPOINTS = .;
    KEEP(*(linkme_GROUNDHOG_RV32_ENTRYPOINTS))
    __stop_linkme_GROUNDHOG_RV32_ENTRYPOINTS = .;
    __start_linkm2_GROUNDHOG_RV32_ENTRYPOINTS = .;
    KEEP(*(linkm2_GROUNDHOG_RV32_ENTRYPOINTS))
    __stop_linkm2_GROUNDHOG_RV32_ENTRYPOINTS = .;

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

  /* Heap grows from __heap_base toward the end of DATA. The allocator
   * checks __heap_end as the bound. The VM initializes sp to the top of
   * guest data memory, so keep the heap below a fixed stack reserve.
   */
  . = ALIGN(8);
  __heap_base = .;
  __stack_top = ORIGIN(DATA) + LENGTH(DATA);
  __stack_bottom = __stack_top - __stack_size;
  __heap_end = __stack_bottom;

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

ASSERT(__heap_base <= __heap_end, "rv32 image leaves no room for heap/stack");
