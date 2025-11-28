use super::gamepad_mappings::Xbox360Layout;
use evdev::{AbsInfo, AttributeSet, Key, UinputAbsSetup, uinput::{VirtualDevice, VirtualDeviceBuilder}};

pub fn create_virtual_gamepad() -> Result<VirtualDevice, Box<dyn std::error::Error>> {
    // Build device using explicit vendor/product and axis list to match Xbox 360 mapping.
    // Note: `with_vendor_id`/`with_product_id` are optional; the current evdev
    // version in use doesn't expose them on `VirtualDeviceBuilder`, so we omit
    // them. The device will still be configured with the exact keys and axes.
    // Build AttributeSet of keys from mapping
    let mut keys = AttributeSet::<Key>::new();
    for &code in Xbox360Layout::BUTTON_CODES.iter() {
        keys.insert(Key::new(code));
    }

    let mut builder = VirtualDeviceBuilder::new()?
        .name("Retro Virtual Xbox 360")
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
