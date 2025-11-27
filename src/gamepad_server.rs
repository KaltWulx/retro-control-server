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
    let mut buf = [0u8; 64];

    loop {
        let (len, _) = socket.recv_from(&mut buf).await?;

        if let Some((buttons, axes)) = parse_gamepad_snapshot(&buf[..len]) {
            log(Verbosity::High, &format!("Gamepad Snapshot: buttons={:?}, axes={:?}", buttons, axes));

            let mut events = Vec::new();
            process_buttons(buttons, &mut events);
            process_axes(axes, &mut events);

            if !events.is_empty() {
                if let Ok(mut dev) = device.lock() {
                    let _ = dev.emit(&events);
                }
            }
        }
    }
}

fn parse_gamepad_snapshot(buf: &[u8]) -> Option<([u8; 12], [i16; 8])> {
    if buf.len() >= 29 && buf[0] == HEADER_GAMEPAD_SNAPSHOT {
        log_data(Verbosity::High, "UDP Gamepad Snapshot", buf);

        let buttons: [u8; 12] = buf[1..13].try_into().ok()?;
        let axes_bytes = &buf[13..29];
        let axes: [i16; 8] = bytemuck::cast_slice(axes_bytes).try_into().ok()?;

        Some((buttons, axes))
    } else {
        None
    }
}

fn process_buttons(buttons: [u8; 12], events: &mut Vec<InputEvent>) {
    for (i, &state) in buttons.iter().enumerate() {
        match i {
            0 => events.push(InputEvent::new(EventType::KEY, Key::BTN_SOUTH.0, state as i32)),  // A
            1 => events.push(InputEvent::new(EventType::KEY, Key::BTN_EAST.0, state as i32)),   // B
            2 => events.push(InputEvent::new(EventType::KEY, Key::BTN_NORTH.0, state as i32)),  // X
            3 => events.push(InputEvent::new(EventType::KEY, Key::BTN_WEST.0, state as i32)),   // Y
            4 => events.push(InputEvent::new(EventType::KEY, Key::BTN_TL.0, state as i32)),     // LB
            5 => events.push(InputEvent::new(EventType::KEY, Key::BTN_TR.0, state as i32)),     // RB
            6 => events.push(InputEvent::new(EventType::KEY, Key::BTN_START.0, state as i32)),  // Start
            7 => events.push(InputEvent::new(EventType::KEY, Key::BTN_SELECT.0, state as i32)), // Back
            8 => events.push(InputEvent::new(EventType::KEY, Key::BTN_THUMBL.0, state as i32)), // Thumb L
            9 => events.push(InputEvent::new(EventType::KEY, Key::BTN_THUMBR.0, state as i32)), // Thumb R
            10 => events.push(InputEvent::new(EventType::KEY, Key::BTN_MODE.0, state as i32)),   // Hotkey
            11 => events.push(InputEvent::new(EventType::KEY, Key::BTN_MODE.0, state as i32)),   // Guide
            _ => {},
        }
    }
}

fn process_axes(axes: [i16; 8], events: &mut Vec<InputEvent>) {
    for (i, &value) in axes.iter().enumerate() {
        match i {
            0 => events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, value as i32)),   // Left X
            1 => events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, value as i32)),   // Left Y
            2 => events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_RX.0, value as i32)),  // Right X
            3 => events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_RY.0, value as i32)),  // Right Y
            4 => events.push(InputEvent::new(EventType::KEY, Key::BTN_THUMBL.0, if value == 0 { 0 } else { 1 })), // Trigger L
            5 => events.push(InputEvent::new(EventType::KEY, Key::BTN_THUMBR.0, if value == 0 { 0 } else { 1 })), // Trigger R
            6 => {  // Hat X
                let scaled = if value < 0 { -1 } else if value > 0 { 1 } else { 0 };
                events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_HAT0X.0, scaled));
            },
            7 => {  // Hat Y
                let scaled = if value < 0 { -1 } else if value > 0 { 1 } else { 0 };
                events.push(InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_HAT0Y.0, scaled));
            },
            _ => {},
        }
    }
}