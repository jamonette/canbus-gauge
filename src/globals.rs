use static_cell::ConstStaticCell;

pub static DISPLAY_SPI_BUF: ConstStaticCell<[u8; 512]> = ConstStaticCell::new([0u8; 512]);
