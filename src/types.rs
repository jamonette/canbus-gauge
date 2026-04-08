use embassy_stm32::gpio::Output;
use embassy_stm32::mode::Blocking;
use embassy_stm32::spi::Spi;
use embassy_stm32::spi::mode::Master;
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use mcp2515::MCP2515;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ILI9341Rgb565;

pub type BlockingSpi = Spi<'static, Blocking, Master>;
pub type ExclusiveBlockingSpiDevice = ExclusiveDevice<BlockingSpi, Output<'static>, NoDelay>;
pub type CanbusController = MCP2515<ExclusiveBlockingSpiDevice>;
pub type DisplaySpiInterface = SpiInterface<'static, ExclusiveBlockingSpiDevice, Output<'static>>;
pub type Display = mipidsi::Display<DisplaySpiInterface, ILI9341Rgb565, Output<'static>>;
