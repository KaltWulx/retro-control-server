use super::xbox360_layout::Xbox360Layout;
use evdev::{AbsInfo, AttributeSet, Key, UinputAbsSetup, uinput::{VirtualDevice, VirtualDeviceBuilder}};

pub fn create_virtual_gamepad() -> Result<VirtualDevice, Box<dyn std::error::Error>> {
    // Build AttributeSet of keys
    let key_array = [
        Key::BTN_SOUTH,  // A
        Key::BTN_EAST,   // B
        Key::BTN_NORTH,  // X
        Key::BTN_WEST,   // Y
        Key::BTN_TL,     // LB
        Key::BTN_TR,     // RB
        Key::BTN_SELECT, // Back
        Key::BTN_START,  // Start
        Key::BTN_MODE,   // Guide
        Key::BTN_THUMBL, // Left Stick Press
        Key::BTN_THUMBR, // Right Stick Press
    ];
    let mut keys = AttributeSet::<Key>::new();
    for &key in &key_array {
        keys.insert(key);
    }

    let mut builder = VirtualDeviceBuilder::new()?
        .name("RetroControl Virtual Gamepad")
        .with_keys(&keys)?;

    // Add absolute axes individually (evdev version provides `with_absolute_axis`).
    let axes = [
        (0, AbsInfo::new(0, Xbox360Layout::STICK_MIN, Xbox360Layout::STICK_MAX, 16, 128, 0)), // ABS_X
        (1, AbsInfo::new(0, Xbox360Layout::STICK_MIN, Xbox360Layout::STICK_MAX, 16, 128, 0)), // ABS_Y
        (3, AbsInfo::new(0, Xbox360Layout::STICK_MIN, Xbox360Layout::STICK_MAX, 16, 128, 0)), // ABS_RX
        (4, AbsInfo::new(0, Xbox360Layout::STICK_MIN, Xbox360Layout::STICK_MAX, 16, 128, 0)), // ABS_RY
        (2, AbsInfo::new(0, Xbox360Layout::TRIGGER_MIN, Xbox360Layout::TRIGGER_MAX, 0, 0, 0)), // ABS_Z
        (5, AbsInfo::new(0, Xbox360Layout::TRIGGER_MIN, Xbox360Layout::TRIGGER_MAX, 0, 0, 0)), // ABS_RZ
        (16, AbsInfo::new(0, Xbox360Layout::HAT_MIN, Xbox360Layout::HAT_MAX, 0, 0, 0)), // ABS_HAT0X
        (17, AbsInfo::new(0, Xbox360Layout::HAT_MIN, Xbox360Layout::HAT_MAX, 0, 0, 0)), // ABS_HAT0Y
    ];

    for (code, info) in axes.iter() {
        let axis = match *code {
            0 => evdev::AbsoluteAxisType::ABS_X,
            1 => evdev::AbsoluteAxisType::ABS_Y,
            2 => evdev::AbsoluteAxisType::ABS_Z,
            3 => evdev::AbsoluteAxisType::ABS_RX,
            4 => evdev::AbsoluteAxisType::ABS_RY,
            5 => evdev::AbsoluteAxisType::ABS_RZ,
            16 => evdev::AbsoluteAxisType::ABS_HAT0X,
            17 => evdev::AbsoluteAxisType::ABS_HAT0Y,
            _ => evdev::AbsoluteAxisType::ABS_MISC,
        };

        let setup = UinputAbsSetup::new(axis, *info);
        builder = builder.with_absolute_axis(&setup)?;
    }

    let device = builder.build()?;
    Ok(device)
}
