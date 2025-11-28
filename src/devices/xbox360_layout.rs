/// Layout EXACTO de un control Xbox 360 real según evtest y xpad.
/// Úsalo para construir un device virtual idéntico.
pub struct Xbox360Layout;

/// Botones reales del control Xbox 360 vía evdev (driver xpad).
///
/// NOTA: El control tiene **11 botones reales**:
/// A, B, X, Y,
/// LB, RB,
/// Back, Start,
/// Guide,
/// Stick Left Press, Stick Right Press
impl Xbox360Layout {
    pub const BUTTON_COUNT: usize = 11;

    /// Botones reales y en orden estándar (A,B,X,Y,LB,RB,Back,Start,Guide,L3,R3)
    pub const BUTTON_CODES: [u16; Self::BUTTON_COUNT] = [
        304, // BTN_SOUTH  (A)
        305, // BTN_EAST   (B)
        307, // BTN_NORTH  (X)
        308, // BTN_WEST   (Y)
        310, // BTN_TL     (LB)
        311, // BTN_TR     (RB)
        314, // BTN_SELECT (Back)
        315, // BTN_START  (Start)
        316, // BTN_MODE   (Guide)
        317, // BTN_THUMBL (Left Stick Press)
        318, // BTN_THUMBR (Right Stick Press)
    ];

    // ----- AXES -----
    // El Xbox 360 tiene:
    //
    //   ABS_X      left stick X      (-32768..32767)
    //   ABS_Y      left stick Y      (-32768..32767)
    //   ABS_RX     right stick X     (-32768..32767)
    //   ABS_RY     right stick Y     (-32768..32767)
    //   ABS_Z      trigger izquierdo (0..255)
    //   ABS_RZ     trigger derecho   (0..255)
    //   ABS_HAT0X  dpad horizontal   [-1, 0, +1]
    //   ABS_HAT0Y  dpad vertical     [-1, 0, +1]
    //
    // Eso son 8 ejes reales.
    pub const AXIS_COUNT: usize = 8;

    pub const AXIS_CODES: [i32; Self::AXIS_COUNT] = [
        0,  // ABS_X     - left stick X
        1,  // ABS_Y     - left stick Y
        3,  // ABS_RX    - right stick X
        4,  // ABS_RY    - right stick Y
        2,  // ABS_Z     - trigger L (0..255)
        5,  // ABS_RZ    - trigger R (0..255)
        16, // ABS_HAT0X - dpad horizontal (-1,0,1)
        17, // ABS_HAT0Y - dpad vertical   (-1,0,1)
    ];

    // Rangos estándar que usa xpad en Linux (evdev)
    pub const STICK_MIN: i32 = -32768;
    pub const STICK_MAX: i32 = 32767;

    pub const TRIGGER_MIN: i32 = 0;
    pub const TRIGGER_MAX: i32 = 255;

    pub const HAT_MIN: i32 = -1;
    pub const HAT_MAX: i32 = 1;

    pub fn button_code(idx: usize) -> Option<u16> {
        Self::BUTTON_CODES.get(idx).copied()
    }

    pub fn axis_code(idx: usize) -> Option<i32> {
        Self::AXIS_CODES.get(idx).copied()
    }
}
