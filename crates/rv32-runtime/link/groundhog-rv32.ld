/* Groundhog RV32 contract linker script.
 *
 * Layout invariants enforced by the hogd lowerer (see
 * docs/rv32/rv32-vm-experimental-design.md, "Rust Fixture And Lowering"):
 *
 *   - .text starts at virtual address 0 (so PC space and VA space match).
 *   - .rodata / .data / .bss live above .text inside the 64 KiB memory limit.
 *   - the heap stops below a reserved stack region at the top of RAM.
 *   - No executable sections other than .text.
 *   - No GOT / PLT / dynamic relocations.
 *   - No init_array / eh_frame.
 *
 * The hogd VM is Harvard-style: PC indexes immutable code; loads and stores
 * address guest data memory. Low data memory corresponding to .text is dead
 * space at runtime — the initial memory image still allocates it.
 */

ENTRY(_start)

__stack_size = 4K;

MEMORY
{
  RAM (rwx) : ORIGIN = 0, LENGTH = 64K
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
  } > RAM

  . = ALIGN(8);
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
  } > RAM

  . = ALIGN(8);
  .data : ALIGN(8)
  {
    *(.sdata .sdata.*)
    *(.data .data.*)
  } > RAM

  . = ALIGN(8);
  .bss (NOLOAD) : ALIGN(8)
  {
    __bss_start = .;
    *(.sbss .sbss.*)
    *(.bss .bss.*)
    *(COMMON)
    __bss_end = .;
  } > RAM

  /* Heap grows from __heap_base toward the end of RAM. The allocator
   * checks __heap_end as the bound. The VM initializes sp to the top of
   * guest memory, so keep the heap below a fixed stack reserve.
   */
  . = ALIGN(8);
  __heap_base = .;
  __stack_top = ORIGIN(RAM) + LENGTH(RAM);
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
