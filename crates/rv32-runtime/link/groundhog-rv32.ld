/* Groundhog RV32 contract linker script.
 *
 * Layout invariants enforced by the hogd lowerer (see
 * docs/rv32/rv32-vm-experimental-design.md, "Rust Fixture And Lowering"):
 *
 *   - .text starts at virtual address 0 (so PC space and VA space match).
 *   - .rodata / .data / .bss live above .text inside the 64 KiB memory limit.
 *   - No executable sections other than .text.
 *   - No GOT / PLT / dynamic relocations.
 *   - No init_array / eh_frame.
 *
 * The hogd VM is Harvard-style: PC indexes immutable code; loads and stores
 * address guest data memory. Low data memory corresponding to .text is dead
 * space at runtime — the initial memory image still allocates it.
 */

ENTRY(_start)

MEMORY
{
  RAM (rwx) : ORIGIN = 0, LENGTH = 64K
}

SECTIONS
{
  .text 0 : ALIGN(4)
  {
    /* _start is the linker entry, used when the artifact dispatch routes
     * method ID 0 here. KEEP it so dead-code elimination can't drop it
     * if the contract never references it by name.
     */
    KEEP(*(.text._start))

    /* Method entry points emitted by the #[contract] macro on rv32.
     * Naming convention: gh_method_<method_id>. Lowerer scans these.
     */
    KEEP(*(.text.gh_method_*))

    *(.text .text.*)
  } > RAM

  . = ALIGN(8);
  .rodata : ALIGN(8)
  {
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
   * checks __heap_end as the bound.
   */
  . = ALIGN(8);
  __heap_base = .;
  __heap_end = ORIGIN(RAM) + LENGTH(RAM);

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
