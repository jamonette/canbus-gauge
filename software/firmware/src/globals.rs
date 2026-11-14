use core::sync::atomic::{AtomicU16, AtomicU32};
use static_cell::ConstStaticCell;

pub static DISPLAY_SPI_BUF: ConstStaticCell<[u8; 512]> = ConstStaticCell::new([0u8; 512]);

// Unfortunately it's not possible to create an atomic parameterized
// with an aliased type for units. Revisit at some point for
// a cleaner solution.
pub static OIL_TEMP_F: AtomicU16 = AtomicU16::new(0);
pub static OIL_TEMP_LAST_RCVD: AtomicU32 = AtomicU32::new(u32::MAX);

pub static OIL_PRESSURE_PSI: AtomicU16 = AtomicU16::new(0);
pub static OIL_PRESSURE_LAST_RCVD: AtomicU32 = AtomicU32::new(u32::MAX);
