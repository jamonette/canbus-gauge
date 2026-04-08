pub mod canbus_config {
    pub const OIL_TEMP_CAN_ID: u16 = 0x3E0;
    pub const OIL_PRESSURE_CAN_ID: u16 = 0x361;
}

pub mod display_config {
    pub const DISPLAY_HEIGHT: u16 = 240;
    pub const DISPLAY_WIDTH: u16 = 320;
    pub const BORDER_MARGIN_X: u16 = 4;
    pub const BORDER_MARGIN_Y: u16 = 4;
    pub const GAUGE_NAME_X: i32 = 16;
    pub const GAUGE_VALUE_X: i32 = 160;
    pub const ROW_0_Y: i32 = 80;
    pub const ROW_1_Y: i32 = 110;
}
