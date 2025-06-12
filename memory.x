# memory.x - 内存布局配置
MEMORY
{
    FLASH    : ORIGIN = 0x08000000, LENGTH = 2048K /* 主闪存，2MB */
    RAM      : ORIGIN = 0x24000000, LENGTH = 512K  /* SRAM1/2/3 (AXI/SRAM) */
    RAM_D3   : ORIGIN = 0x38000000, LENGTH = 128K  /* SRAM4 (D3域) */
}

SECTIONS
{
    .ram_d3 :
    {
        *(.ram_d3)
    } > RAM_D3 AT > FLASH
}