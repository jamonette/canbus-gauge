use embassy_time::{Duration, Ticker};
use embedded_graphics::primitives::Line;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::constants::display_config;
use crate::types::Display;

#[embassy_executor::task]
pub async fn display_update_task(mut _display: Display) {
    // Update display @ 10 Hz
    let mut ticker = Ticker::every(Duration::from_millis(100));
    loop {
        ticker.next().await;
    }
}

pub fn draw_initial_ui(display: &mut Display) {
    // The Rgb565::new() params seem to be B, G, R for some reason. I think
    // it has to do with the macro resolution in the underlying library,
    // but ignoring it for now.
    let text_orange = MonoTextStyle::new(&FONT_10X20, Rgb565::new(0, 33, 31));
    let text_white = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);

    display.clear(Rgb565::BLACK).unwrap();

    Rectangle::new(
        Point::new(
            display_config::BORDER_MARGIN_X as i32,
            display_config::BORDER_MARGIN_Y as i32,
        ),
        Size::new(
            (display_config::DISPLAY_WIDTH - (2 * display_config::BORDER_MARGIN_X)) as u32,
            (display_config::DISPLAY_HEIGHT - (2 * display_config::BORDER_MARGIN_Y)) as u32,
        ),
    )
    .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 2))
    .draw(display)
    .unwrap();

    Text::new("CANBUS Gauge v0.0.1", Point::new(16, 35), text_orange)
        .draw(display)
        .unwrap();

    const DIVIDER_Y: i32 = 45;
    Line::new(
        Point::new(display_config::BORDER_MARGIN_X as i32, DIVIDER_Y),
        Point::new(
            (display_config::DISPLAY_WIDTH - display_config::BORDER_MARGIN_X) as i32,
            DIVIDER_Y,
        ),
    )
    .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
    .draw(display)
    .unwrap();

    Text::new(
        "Oil pressure:",
        Point::new(display_config::GAUGE_NAME_X, display_config::ROW_0_Y),
        text_orange,
    )
    .draw(display)
    .unwrap();

    Text::new(
        "Oil temp:",
        Point::new(display_config::GAUGE_NAME_X, display_config::ROW_1_Y),
        text_orange,
    )
    .draw(display)
    .unwrap();

    Text::new(
        "--- psi",
        Point::new(display_config::GAUGE_VALUE_X, display_config::ROW_0_Y),
        text_white,
    )
    .draw(display)
    .unwrap();

    Text::new(
        "--- f",
        Point::new(display_config::GAUGE_VALUE_X, display_config::ROW_1_Y),
        text_white,
    )
    .draw(display)
    .unwrap();
}
