use evdev::{
    AttributeSet, Key, RelativeAxisType,
    uinput::{VirtualDevice, VirtualDeviceBuilder},
};

pub fn create_virtual_mouse() -> Result<VirtualDevice, Box<dyn std::error::Error>> {
    let mut keys = AttributeSet::<Key>::new();
    keys.insert(Key::BTN_LEFT);
    keys.insert(Key::BTN_RIGHT);
    keys.insert(Key::BTN_MIDDLE);

    let mut rel_axes = AttributeSet::<RelativeAxisType>::new();
    rel_axes.insert(RelativeAxisType::REL_X);
    rel_axes.insert(RelativeAxisType::REL_Y);
    rel_axes.insert(RelativeAxisType::REL_WHEEL);

    let device = VirtualDeviceBuilder::new()?
        .name("Retro Control Mouse")
        .with_keys(&keys)?
        .with_relative_axes(&rel_axes)?
        .build()?;

    Ok(device)
}

pub fn create_virtual_keyboard() -> Result<VirtualDevice, Box<dyn std::error::Error>> {
    let mut keys = AttributeSet::<Key>::new();

    for i in 0..255 {
        keys.insert(Key::new(i));
    }

    let device = VirtualDeviceBuilder::new()?
        .name("Retro Control Keyboard")
        .with_keys(&keys)?
        .build()?;

    Ok(device)
}
