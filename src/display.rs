use core::fmt::Write as _;
use core::sync::atomic::Ordering;
use embassy_time::{Duration, Instant, Ticker};
use embedded_graphics::primitives::Line;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use heapless::String;

use crate::types::Display;
use crate::{constants, constants::display_config, globals};

// The Rgb565::new() params seem to be B, G, R for some reason. I think
// it has to do with the macro resolution in the underlying library,
// but ignoring it for now.
static TEXT_ORANGE: MonoTextStyle<Rgb565> = MonoTextStyle::new(&FONT_10X20, Rgb565::new(0, 33, 31));
static TEXT_WHITE: MonoTextStyle<Rgb565> = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);

#[embassy_executor::task]
pub async fn display_update_task(mut display: Display) {
    // Update display @ 10 Hz
    let mut ticker = Ticker::every(Duration::from_millis(100));

    loop {
        ticker.next().await;
        let now = Instant::now().as_millis() as u32;

        let empty_value = "---";
        let oil_temp_str: String<16> =
            if value_is_stale(now, globals::OIL_TEMP_LAST_RCVD.load(Ordering::Relaxed)) {
                String::try_from(empty_value).unwrap()
            } else {
                let mut str: String<16> = String::new();
                core::write!(str, "{}", globals::OIL_TEMP_F.load(Ordering::Relaxed)).unwrap();
                str
            };

        let oil_pressure_str: String<16> =
            if value_is_stale(now, globals::OIL_PRESSURE_LAST_RCVD.load(Ordering::Relaxed)) {
                String::try_from(empty_value).unwrap()
            } else {
                let mut str: String<16> = String::new();
                core::write!(str, "{}", globals::OIL_PRESSURE_PSI.load(Ordering::Relaxed)).unwrap();
                str
            };

        update_value(
            &mut display,
            display_config::GAUGE_VALUE_X,
            display_config::ROW_0_Y,
            display_config::GAUGE_VALUE_X,
            display_config::ROW_0_Y - display_config::GAUGE_FONT_HEIGHT as i32,
            display_config::GAUGE_VALUE_WIDTH,
            display_config::GAUGE_VALUE_WIDTH,
            oil_temp_str,
        );

        update_value(
            &mut display,
            display_config::GAUGE_VALUE_X,
            display_config::ROW_1_Y,
            display_config::GAUGE_VALUE_X,
            display_config::ROW_1_Y - display_config::GAUGE_FONT_HEIGHT as i32,
            display_config::GAUGE_VALUE_WIDTH,
            display_config::GAUGE_VALUE_WIDTH,
            oil_pressure_str,
        );

        ticker.next().await;
    }
}

fn value_is_stale(now_ms: u32, last_ms: u32) -> bool {
    last_ms == u32::MAX || now_ms - last_ms > constants::LAST_RECEIVED_THRESHOLD_MS
}

#[allow(clippy::too_many_arguments)]
pub fn update_value(
    display: &mut Display,
    text_x: i32,
    text_y: i32,
    bounding_box_x: i32,
    bounding_box_y: i32,
    width: u32,
    height: u32,
    value: String<16>,
) {
    // Draw over previous value
    Rectangle::new(
        Point::new(bounding_box_x, bounding_box_y),
        Size::new(width, height),
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
    .draw(display)
    .ok();

    Text::new(&value, Point::new(text_x, text_y), TEXT_WHITE)
        .draw(display)
        .unwrap();
}

pub fn draw_initial_ui(display: &mut Display) {
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

    Text::new("CANBUS Gauge v0.0.1", Point::new(16, 35), TEXT_ORANGE)
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
        TEXT_ORANGE,
    )
    .draw(display)
    .unwrap();

    Text::new(
        "Oil temp:",
        Point::new(display_config::GAUGE_NAME_X, display_config::ROW_1_Y),
        TEXT_ORANGE,
    )
    .draw(display)
    .unwrap();

    Text::new(
        "--- psi",
        Point::new(display_config::GAUGE_VALUE_X, display_config::ROW_0_Y),
        TEXT_WHITE,
    )
    .draw(display)
    .unwrap();

    Text::new(
        "--- f",
        Point::new(display_config::GAUGE_VALUE_X, display_config::ROW_1_Y),
        TEXT_WHITE,
    )
    .draw(display)
    .unwrap();
}
