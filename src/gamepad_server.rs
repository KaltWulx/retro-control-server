use crate::logger::{log, log_data, Verbosity};
use crate::protocol::HEADER_GAMEPAD_SNAPSHOT;
use evdev::{AbsoluteAxisType, EventType, InputEvent, Key, uinput::VirtualDevice};
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;

pub async fn run_udp_gamepad_server(
    port: u16,
    device: Arc<Mutex<VirtualDevice>>,
) -> std::io::Result<()> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
    let mut buf = [0u8; 64]; // Buffer larger than needed

    loop {
        let (len, _) = socket.recv_from(&mut buf).await?;

        if len >= 29 && buf[0] == HEADER_GAMEPAD_SNAPSHOT {
            log_data(Verbosity::High, "UDP Gamepad Snapshot", &buf[..len]);

            // Parse buttons: 12 bytes starting from index 1
            let buttons: [u8; 12] = buf[1..13].try_into().unwrap();

            // Parse axes: 16 bytes starting from index 13, convert to [i16; 8]
            let axes_bytes = &buf[13..29];
            let axes: [i16; 8] = bytemuck::cast_slice(axes_bytes).try_into().unwrap();

            log(Verbosity::High, &format!("Gamepad Snapshot: buttons={:?}, axes={:?}", buttons, axes));

            // Process the snapshot
            process_gamepad_snapshot(buttons, axes, &device);
        }
    }
}

fn process_gamepad_snapshot(buttons: [u8; 12], axes: [i16; 8], device: &Arc<Mutex<VirtualDevice>>) {
    let mut events = Vec::new();

    // Button mappings
    let button_keys = [
        Key::BTN_SOUTH,  // A
        Key::BTN_EAST,   // B
        Key::BTN_NORTH,  // X
        Key::BTN_WEST,   // Y
        Key::BTN_TL,     // LB
        Key::BTN_TR,     // RB
        Key::BTN_START,  // Start
        Key::BTN_SELECT, // Back
        Key::BTN_THUMBL, // Thumb L
        Key::BTN_THUMBR, // Thumb R
        Key::BTN_MODE,   // Hotkey
        Key::BTN_MODE,   // Guide (same as hotkey)
    ];

    for (i, &state) in buttons.iter().enumerate() {
        if let Some(key) = button_keys.get(i) {
            events.push(InputEvent::new(EventType::KEY, key.0, state as i32));
        }
    }

    for (i, &value) in axes.iter().enumerate() {
        match i {
            0 => events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, value as i32)),
            1 => events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, value as i32)),
            2 => events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_RX.0, value as i32)),
            3 => events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_RY.0, value as i32)),
            4 => events.push(InputEvent::new(EventType::KEY, Key::BTN_THUMBL.0, if value == 0 { 0 } else { 1 })),
            5 => events.push(InputEvent::new(EventType::KEY, Key::BTN_THUMBR.0, if value == 0 { 0 } else { 1 })),
            6 => {
                let scaled = if value < 0 { -1 } else if value > 0 { 1 } else { 0 };
                events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_HAT0X.0, scaled));
            },
            7 => {
                let scaled = if value < 0 { -1 } else if value > 0 { 1 } else { 0 };
                events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_HAT0Y.0, scaled));
            },
            _ => {},
        }
    }

    if !events.is_empty() {
        if let Ok(mut dev) = device.lock() {
            let _ = dev.emit(&events);
        }
    }
}