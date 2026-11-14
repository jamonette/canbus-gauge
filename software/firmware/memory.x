# STM32F411CEU6 memory layout.
#
#   - Flash: 512 KiB starting at 0x0800_0000
#   - RAM:   128 KiB starting at 0x2000_0000

MEMORY {
    FLASH : ORIGIN = 0x08000000, LENGTH = 512K
    RAM   : ORIGIN = 0x20000000, LENGTH = 128K
}
