#![no_std]
#![no_main]

mod canbus;
mod constants;
mod display;
mod globals;
mod types;

use constants::{canbus_config, display_config};
use defmt::debug;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::rcc::{APBPrescaler, Hse, HseMode, Pll, PllMul, PllPreDiv, PllSource, Sysclk};
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_time::Delay;
use embedded_can::{Id, StandardId};
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
async fn main(spawner: Spawner) {
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

    let display_interface = mipidsi::interface::SpiInterface::new(
        display_spi_device,
        display_dc,
        globals::DISPLAY_SPI_BUF.take(), // buffer needs a static lifetime, so keep it in a StaticCell
    );

    let mut display = mipidsi::Builder::new(ILI9341Rgb565, display_interface)
        .display_size(
            display_config::DISPLAY_HEIGHT,
            display_config::DISPLAY_WIDTH,
        ) // Args are backwards here since display is in landscape mode
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

    let oil_temp_can_id = Id::Standard(StandardId::new(canbus_config::OIL_TEMP_CAN_ID).unwrap());
    let oil_pressure_can_id =
        Id::Standard(StandardId::new(canbus_config::OIL_PRESSURE_CAN_ID).unwrap());

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

    display::draw_initial_ui(&mut display);

    spawner.spawn(display::display_update_task(display).expect("Failed to spawn display_task"));
    spawner.spawn(
        canbus::canbus_reader_task(can_controller).expect("Failed to spawn canbus_reader task"),
    );

    #[allow(clippy::empty_loop)]
    loop {}
}
