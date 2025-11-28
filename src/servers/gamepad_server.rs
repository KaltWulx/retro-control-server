use crate::logger::{log, log_data, Verbosity};
use crate::protocol::HEADER_GAMEPAD_SNAPSHOT;
use crate::devices::Xbox360Layout;
use evdev::{EventType, InputEvent, Key, uinput::VirtualDevice};
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
            emit_events(&device, &events);
        }
    }
}

fn parse_gamepad_snapshot(buf: &[u8]) -> Option<([u8; 12], [i16; 8])> {
    if buf.len() >= 19 && buf[0] == HEADER_GAMEPAD_SNAPSHOT {
        log_data(Verbosity::High, "UDP Gamepad Snapshot", buf);

        // Buttons packed into 2 bytes (16 bits)
        let button_bits = u16::from_le_bytes([buf[1], buf[2]]);
        let mut buttons = [0u8; 12];
        for i in 0..12 {
            buttons[i] = ((button_bits >> i) & 1) as u8;
        }

        let mut axes = [0i16; 8];
        for i in 0..8 {
            let start = 3 + i * 2;
            axes[i] = i16::from_le_bytes([buf[start], buf[start + 1]]);
        }

        Some((buttons, axes))
    } else {
        None
    }
}

fn process_buttons(buttons: [u8; 12], events: &mut Vec<InputEvent>) {
    for (i, &state) in buttons.iter().enumerate() {
        if let Some(code) = Xbox360Layout::button_code(i) {
            // Use Key::new to create a Key from the numeric evdev code
            let key = Key::new(code);
            events.push(InputEvent::new(EventType::KEY, key.0, state as i32));
        }
    }
}

fn process_axes(axes: [i16; 8], events: &mut Vec<InputEvent>) {
    // Threshold to consider a trigger 'pressed' for digital KEY emission
    const TRIGGER_DIGITAL_THRESHOLD: i32 = 10;

    for (i, &value) in axes.iter().enumerate() {
        if let Some(code) = Xbox360Layout::axis_code(i) {
            match i {
                4 | 5 => {
                    // Triggers: emit ABS (analog) and also emit a digital KEY when above threshold
                    let abs_code = code as u16;
                    let abs_value = value as i32;
                    events.push(InputEvent::new(EventType::ABSOLUTE, abs_code, abs_value));

                    let key_val = if abs_value > TRIGGER_DIGITAL_THRESHOLD { 1 } else { 0 };
                    let key_code = if i == 4 { Key::BTN_THUMBL.0 } else { Key::BTN_THUMBR.0 };
                    events.push(InputEvent::new(EventType::KEY, key_code, key_val));
                }
                6 | 7 => { // Hat axes: scale to -1/0/1
                    let scaled = if value < 0 { -1 } else if value > 0 { 1 } else { 0 };
                    events.push(InputEvent::new(EventType::ABSOLUTE, code as u16, scaled));
                }
                _ => events.push(InputEvent::new(EventType::ABSOLUTE, code as u16, value as i32)),
            }
        }
        // else: no mapping for this axis index, ignore
    }
}

fn emit_events(device: &Arc<Mutex<VirtualDevice>>, events: &[InputEvent]) {
    if !events.is_empty() {
        if let Ok(mut dev) = device.lock() {
            let _ = dev.emit(events);
        }
    }
}
