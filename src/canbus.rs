use core::sync::atomic::Ordering;
use defmt::warn;
use embassy_futures::yield_now;
use embedded_can::{Frame, Id, StandardId, blocking::Can};
use {defmt_rtt as _, panic_probe as _};

use crate::constants::canbus_config;
use crate::globals;
use crate::types::CanbusController;

#[embassy_executor::task]
pub async fn canbus_reader_task(mut can_controller: CanbusController) {
    let oil_temp_can_id =
        Id::Standard(StandardId::new(canbus_config::OIL_TEMP_CAN_ID).expect("Invalid CAN id"));
    let oil_pressure_can_id =
        Id::Standard(StandardId::new(canbus_config::OIL_PRESSURE_CAN_ID).expect("Invalid CAN id"));

    loop {
        match can_controller.receive() {
            // CAN frame received — parse and update shared state.
            // See: `./docs/Haltech-CAN-protocol-reference-v2.pdf`
            Ok(frame) => {
                let frame_can_id = frame.id();
                let data = frame.data();

                if frame_can_id == oil_temp_can_id {
                    match data.get(6..8) {
                        Some([b1, b2]) => {
                            // Haltech protocol is big-endian
                            let raw_temp = u16::from_be_bytes([*b1, *b2]);
                            let kelvin = raw_temp / 10;
                            let celsius = kelvin - 273;
                            let fahrenheit = (celsius * 9 / 5) + 32;
                            globals::OIL_TEMP_F.store(fahrenheit, Ordering::Relaxed);
                        }
                        _ => warn!("Received invalid oil temp frame {:x}", data),
                    }
                } else if frame_can_id == oil_pressure_can_id {
                    // TODO
                }
            }
            Err(mcp2515::error::Error::NoMessage) => yield_now().await,
            Err(_) => {
                warn!("MCP2515 receive error"); // TODO: add fmt impl for MCP error
                yield_now().await;
            }
        }
    }
}
