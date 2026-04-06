# STM32F103C8T6 memory layout.
#
#   - Flash: 64 KiB starting at 0x0800_0000
#   - RAM:   20 KiB starting at 0x2000_0000
#
# Note: depending on the part, the STM might have up to
# 128KiB of flash, in which case increase flash length to 128K.

MEMORY {
    FLASH : ORIGIN = 0x08000000, LENGTH = 128K
    RAM   : ORIGIN = 0x20000000, LENGTH = 20K
}
