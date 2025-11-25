use crate::protocol::{MODE_GAMEPAD, MODE_MOUSE_KEYBOARD};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    MouseKeyboard,
    Gamepad,
}

impl InputMode {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            MODE_MOUSE_KEYBOARD => Some(InputMode::MouseKeyboard),
            MODE_GAMEPAD => Some(InputMode::Gamepad),
            _ => None,
        }
    }
}
