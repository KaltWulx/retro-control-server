use evdev::{
    AbsInfo, AbsoluteAxisType, AttributeSet, Key, UinputAbsSetup,
    uinput::{VirtualDevice, VirtualDeviceBuilder},
};

pub fn create_virtual_gamepad() -> Result<VirtualDevice, Box<dyn std::error::Error>> {
    let mut keys = AttributeSet::<Key>::new();
    keys.insert(Key::BTN_SOUTH);
    keys.insert(Key::BTN_EAST);
    keys.insert(Key::BTN_NORTH);
    keys.insert(Key::BTN_WEST);
    keys.insert(Key::BTN_TL);
    keys.insert(Key::BTN_TR);
    keys.insert(Key::BTN_START);
    keys.insert(Key::BTN_SELECT);

    let mut builder = VirtualDeviceBuilder::new()?
        .name("Retro Control Gamepad")
        .with_keys(&keys)?;

    let abs_axes = [
        AbsoluteAxisType::ABS_X,
        AbsoluteAxisType::ABS_Y,
        AbsoluteAxisType::ABS_RX,
        AbsoluteAxisType::ABS_RY,
        AbsoluteAxisType::ABS_Z,
        AbsoluteAxisType::ABS_RZ,
        AbsoluteAxisType::ABS_HAT0X,
        AbsoluteAxisType::ABS_HAT0Y,
    ];

    for axis in abs_axes {
        let config = match axis {
            AbsoluteAxisType::ABS_X
            | AbsoluteAxisType::ABS_Y
            | AbsoluteAxisType::ABS_RX
            | AbsoluteAxisType::ABS_RY => AbsInfo::new(0, -32768, 32767, 16, 128, 0),
            AbsoluteAxisType::ABS_Z | AbsoluteAxisType::ABS_RZ => AbsInfo::new(0, 0, 255, 0, 0, 0),
            AbsoluteAxisType::ABS_HAT0X | AbsoluteAxisType::ABS_HAT0Y => {
                AbsInfo::new(0, -1, 1, 0, 0, 0)
            }
            _ => AbsInfo::new(0, -32768, 32767, 0, 0, 0),
        };

        let setup = UinputAbsSetup::new(axis, config);
        builder = builder.with_absolute_axis(&setup)?;
    }

    let device = builder.build()?;

    Ok(device)
}
