// Network packet headers
pub const HEADER_MOUSE: u8 = 0x20;
pub const HEADER_KEYBOARD: u8 = 0x10;
pub const HEADER_MODE_SWITCH: u8 = 0x30;
pub const HEADER_MODE_ACK: u8 = 0x31;
pub const HEADER_GAMEPAD_SNAPSHOT: u8 = 0x42;
pub const HEADER_DISCOVERY: u8 = 0x50;

// Input mode identifiers
pub const MODE_MOUSE_KEYBOARD: u8 = 0x01;
pub const MODE_GAMEPAD: u8 = 0x02;

// Discovery broadcast configuration
pub const DISCOVERY_PORT: u16 = 5557;
pub const DISCOVERY_INTERVAL_MS: u64 = 2000;
