#![no_std]
#![no_main]

use defmt::debug;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::rcc::{APBPrescaler, Hse, HseMode, Pll, PllMul, PllPreDiv, PllSource, Sysclk};
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_time::Delay;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, PrimitiveStyle, Rectangle},
    text::Text,
};
use embedded_hal_bus::spi::ExclusiveDevice;
use mipidsi::{
    models::ILI9341Rgb565,
    options::{Orientation, Rotation},
};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    debug!("Starting CANBUS gauge");

    // Use dev board's external oscillator @ 8 MHz
    let mut config = embassy_stm32::Config::default();
    config.rcc.hse = Some(Hse {
        freq: Hertz(8_000_000),
        mode: HseMode::Oscillator,
    });

    // Set clock divider @ 1x, PLL @ 9x to yield STM max clock of 72 MHz.
    config.rcc.pll = Some(Pll {
        src: PllSource::HSE,
        prediv: PllPreDiv::DIV1,
        mul: PllMul::MUL9,
    });

    config.rcc.sys = Sysclk::PLL1_P;

    // Highest supported SPI clock for ILI9341 is 40 MHz, so get as close to
    // that as we can. Use a 0.5x prescaler for APBI, which owns the SPI
    // interface to the display.
    config.rcc.apb1_pre = APBPrescaler::DIV2;

    let peripherals = embassy_stm32::init(config);
    debug!("CPU running @ 72 MHz");

    // Display setup for TPM408-2.8 (uses ILI9341 driver)
    let mut display_spi_config = SpiConfig::default();
    display_spi_config.frequency = Hertz(36_000_000);
    let mut display_spi_buf = [0u8; 512];
    let display_spi = Spi::new_blocking_txonly(
        peripherals.SPI1,
        peripherals.PA5,
        peripherals.PA7,
        display_spi_config,
    );

    debug!("SPI1 for ILI9341 display initialized @ 36 MHz");

    let display_chip_select = Output::new(peripherals.PA4, Level::High, Speed::VeryHigh);
    let display_dc = Output::new(peripherals.PB0, Level::Low, Speed::VeryHigh);
    let display_reset = Output::new(peripherals.PB1, Level::High, Speed::VeryHigh);
    let _display_backlight = Output::new(peripherals.PB10, Level::High, Speed::Low);
    let display_device = ExclusiveDevice::new_no_delay(display_spi, display_chip_select)
        .expect("Failed to init display device");
    let display_interface =
        mipidsi::interface::SpiInterface::new(display_device, display_dc, &mut display_spi_buf);

    mod display_size {
        pub const HEIGHT: u16 = 240;
        pub const WIDTH: u16 = 320;
    }

    let mut display = mipidsi::Builder::new(ILI9341Rgb565, display_interface)
        .display_size(display_size::HEIGHT, display_size::WIDTH) // args backwards since using landscape
        .orientation(
            Orientation::new()
                .rotate(Rotation::Deg270)
                .flip_horizontal(),
        )
        .reset_pin(display_reset)
        .init(&mut Delay)
        .expect("Failed to init display");

    debug!("Display initialized");

    // The Rgb565::new() params seem to be B, G, R for some reason. I think
    // it has to do with the macro resolution in the underlying library,
    // but ignoring it for now.
    let text_orange = MonoTextStyle::new(&FONT_10X20, Rgb565::new(0, 33, 31));
    let text_white = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);

    display.clear(Rgb565::BLACK).unwrap();

    mod display_coords {
        pub const BORDER_MARGIN_X: u16 = 4;
        pub const BORDER_MARGIN_Y: u16 = 4;
        pub const GAUGE_NAME_X: i32 = 16;
        pub const GAUGE_VALUE_X: i32 = 160;
        pub const ROW_0_Y: i32 = 80;
        pub const ROW_1_Y: i32 = 110;
    }

    Rectangle::new(
        Point::new(
            display_coords::BORDER_MARGIN_X as i32,
            display_coords::BORDER_MARGIN_Y as i32,
        ),
        Size::new(
            (display_size::WIDTH - (2 * display_coords::BORDER_MARGIN_X)) as u32,
            (display_size::HEIGHT - (2 * display_coords::BORDER_MARGIN_Y)) as u32,
        ),
    )
    .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 2))
    .draw(&mut display)
    .unwrap();

    Text::new("CANBUS Gauge v0.0.1", Point::new(16, 35), text_orange)
        .draw(&mut display)
        .unwrap();

    const DIVIDER_Y: i32 = 45;
    Line::new(
        Point::new(display_coords::BORDER_MARGIN_X as i32, DIVIDER_Y),
        Point::new(
            (display_size::WIDTH - display_coords::BORDER_MARGIN_X) as i32,
            DIVIDER_Y,
        ),
    )
    .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
    .draw(&mut display)
    .unwrap();

    Text::new(
        "Oil pressure:",
        Point::new(display_coords::GAUGE_NAME_X, display_coords::ROW_0_Y),
        text_orange,
    )
    .draw(&mut display)
    .unwrap();
    Text::new(
        "Oil temp:",
        Point::new(display_coords::GAUGE_NAME_X, display_coords::ROW_1_Y),
        text_orange,
    )
    .draw(&mut display)
    .unwrap();

    Text::new(
        "--- psi",
        Point::new(display_coords::GAUGE_VALUE_X, display_coords::ROW_0_Y),
        text_white,
    )
    .draw(&mut display)
    .unwrap();
    Text::new(
        "--- f",
        Point::new(display_coords::GAUGE_VALUE_X, display_coords::ROW_1_Y),
        text_white,
    )
    .draw(&mut display)
    .unwrap();

    debug!("Entering main loop");

    #[allow(clippy::empty_loop)]
    loop {}
}
