#![no_std]
#![no_main]

use defmt::debug;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::rcc::{APBPrescaler, Hse, HseMode, Pll, PllMul, PllPreDiv, PllSource, Sysclk};
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_time::Delay;
use embedded_can::{Id, StandardId};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, PrimitiveStyle, Rectangle},
    text::Text,
};
use embedded_hal_bus::spi::ExclusiveDevice;
use mcp2515::{
    CanSpeed, MCP2515, McpSpeed,
    filter::{RxFilter, RxMask},
    regs::OpMode,
};
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

    // Set clock divider @ 1x, PLL @ 9x to yield STM32F103C8T6 max clock of 72 MHz
    config.rcc.pll = Some(Pll {
        src: PllSource::HSE,
        prediv: PllPreDiv::DIV1,
        mul: PllMul::MUL9,
    });

    config.rcc.sys = Sysclk::PLL1_P;

    // Highest supported clock for APB1 is 36 MHz, so use a 0.5x prescaler.
    // APB2 can run at the full system clock of 72 MHz.
    config.rcc.apb1_pre = APBPrescaler::DIV2;
    config.rcc.apb2_pre = APBPrescaler::DIV1;

    let peripherals = embassy_stm32::init(config);
    debug!("CPU running @ 72 MHz");

    let mut spi1_config = SpiConfig::default();
    spi1_config.frequency = Hertz(36_000_000);
    let spi1_bus = Spi::new_blocking_txonly(
        peripherals.SPI1,
        peripherals.PA5,
        peripherals.PA7,
        spi1_config,
    );
    debug!("SPI1 initialized @ 36 MHz");

    // MCP2515 supports a max SPI clock of 10 MHz, so use that here
    let mut spi2_config = SpiConfig::default();
    spi2_config.frequency = Hertz(10_000_000);
    let spi2_bus = Spi::new_blocking(
        peripherals.SPI2,
        peripherals.PB13,
        peripherals.PB15,
        peripherals.PB14,
        spi2_config,
    );
    debug!("SPI2 initialized @ 10 MHz");

    // Display setup for TPM408-2.8 (uses ILI9341 driver)
    //
    // TODO:
    //   ILI9341 can do SPI @ 40 MHz, but APB1 maxes out at 36 MHz.
    //   Consider modifying the hardware so that the display uses SPI2, which
    //   lives on APB2, to allow the ILI9341 to run at its max SPI clock speed
    //   since the MCP2515 can only do a max of 10 MHz anyway.
    let display_chip_select = Output::new(peripherals.PA4, Level::High, Speed::VeryHigh);
    let display_dc = Output::new(peripherals.PB0, Level::Low, Speed::VeryHigh);
    let display_reset = Output::new(peripherals.PB1, Level::High, Speed::VeryHigh);
    let _display_backlight = Output::new(peripherals.PB10, Level::High, Speed::Low);
    let display_spi_device = ExclusiveDevice::new_no_delay(spi1_bus, display_chip_select)
        .expect("Failed to init display SPI device");

    let mut display_spi_buf = [0u8; 512];
    let display_interface =
        mipidsi::interface::SpiInterface::new(display_spi_device, display_dc, &mut display_spi_buf);

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
        .expect("Failed to init display (ILI9341)");

    debug!("Display initialized");

    let can_controller_chip_select = Output::new(peripherals.PB12, Level::High, Speed::VeryHigh);
    let can_controller_spi_device =
        ExclusiveDevice::new_no_delay(spi2_bus, can_controller_chip_select)
            .expect("Failed to init SPI device for CAN controller");
    let mut can_controller = MCP2515::new(can_controller_spi_device);
    const CANBUS_SPEED: CanSpeed = CanSpeed::Kbps1000; // Match the Haltech ECU's native bus speed
    can_controller
        .init(
            &mut Delay,
            mcp2515::Settings {
                mode: OpMode::Normal,
                can_speed: CANBUS_SPEED,
                mcp_speed: McpSpeed::MHz8, // External oscillator on MCP dev board is 8 MHz
                clkout_en: false,
            },
        )
        .expect("MPC2515: Failed to init CAN controller");

    let oil_temp_can_id = Id::Standard(StandardId::new(0x3E0).unwrap());
    let oil_pressure_can_id = Id::Standard(StandardId::new(0x361).unwrap());

    // Configure the CAN controller frame filter. This drops all frames before
    // they hit the MCP hardware receive buffers, except those explicitly allowed
    // by the filter. This greatly reduces SPI throughput and CPU workload.
    //
    // The MCP contains two receive buffers and 6 filters, configured such that:
    //
    //      RXB0 (high priority) is governed by RXF0, RXF1, and RXM0
    //      RXB1 (low priority) is governed by RXF2, RXF3, RXF4, RXF5, and RXM1
    //
    // RXM above describes an additional mask that also controls filter matching,
    // such that `(id & mask) == (filter & mask)`.
    //
    // So, to enable the filters to apply all 11 bits of a standard CAN id
    // (aka, requiring an exact match), set each mask to 0x7FF.
    //
    // Note that in its default configuration (BUKT enable), when RXB0 is full,
    // frames automatically roll over to RXB1 _regardless of the RXB1 filters_.
    // In our case, this is the desired behavior. This means that both RXB0 and
    // RXB1 respect the first two RXF filters, but the remaining 4 filters only
    // apply to the second RX buffer, which slightly limits throughput of filtered
    // frames when using more than two.
    //
    // ... at least I think I have that right. See the MCP2515 datasheet in this repo
    // for details.

    can_controller
        .set_mode(OpMode::Configuration)
        .expect("MCP2515: Failed to enter config mode");

    const FILTER_MASK: u16 = 0x7FF;
    let filter_mask_id = Id::Standard(StandardId::new(FILTER_MASK).unwrap());
    can_controller
        .set_mask(RxMask::Mask0, filter_mask_id)
        .expect("MCP2515: Failed to set CAN filter Mask0");
    can_controller
        .set_mask(RxMask::Mask1, filter_mask_id)
        .expect("MCP2515: Failed to set CAN filter Mask1");
    can_controller
        .set_filter(RxFilter::F0, oil_temp_can_id)
        .expect("MCP2515: Failed to set CAN filter 0");
    can_controller
        .set_filter(RxFilter::F1, oil_pressure_can_id)
        .expect("MCP2515: Failed to set CAN filter 1");
    can_controller
        .set_filter(RxFilter::F2, oil_temp_can_id)
        .expect("MCP2515: Failed to set CAN filter 2");
    can_controller
        .set_filter(RxFilter::F3, oil_pressure_can_id)
        .expect("MCP2515: Failed to set CAN filter 3");

    can_controller
        .set_mode(OpMode::Normal)
        .expect("MCP2515: Failed to enter normal mode");
    debug!("MCP2515: Initialization complete");

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
