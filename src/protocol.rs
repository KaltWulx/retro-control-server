// Headers y constantes del protocolo de red
pub const HEADER_MOUSE: u8 = 0x20;
pub const HEADER_KEYBOARD: u8 = 0x10;
pub const HEADER_MODE_SWITCH: u8 = 0x30;
pub const HEADER_MODE_ACK: u8 = 0x31;
pub const HEADER_GAMEPAD_AXIS: u8 = 0x40;
pub const HEADER_GAMEPAD_BUTTON: u8 = 0x41;
pub const MODE_MOUSE_KEYBOARD: u8 = 0x01;
pub const MODE_GAMEPAD: u8 = 0x02;

pub const GAMEPAD_AXIS_LEFT_X: u8 = 0x01;
pub const GAMEPAD_AXIS_LEFT_Y: u8 = 0x02;
pub const GAMEPAD_AXIS_RIGHT_X: u8 = 0x03;
pub const GAMEPAD_AXIS_RIGHT_Y: u8 = 0x04;
pub const GAMEPAD_AXIS_TRIGGER_L: u8 = 0x05;
pub const GAMEPAD_AXIS_TRIGGER_R: u8 = 0x06;

pub const GAMEPAD_BUTTON_A: u8 = 0x01;
pub const GAMEPAD_BUTTON_B: u8 = 0x02;
pub const GAMEPAD_BUTTON_X: u8 = 0x03;
pub const GAMEPAD_BUTTON_Y: u8 = 0x04;
pub const GAMEPAD_BUTTON_LB: u8 = 0x05;
pub const GAMEPAD_BUTTON_RB: u8 = 0x06;
pub const GAMEPAD_BUTTON_START: u8 = 0x07;
pub const GAMEPAD_BUTTON_BACK: u8 = 0x08;
