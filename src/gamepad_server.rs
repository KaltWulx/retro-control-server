use crate::logger::{log, log_data, Verbosity};
use crate::protocol::HEADER_GAMEPAD_SNAPSHOT;
use evdev::{AbsoluteAxisType, EventType, InputEvent, Key, uinput::VirtualDevice};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tokio::sync::Notify;

pub async fn run_udp_gamepad_server(
    port: u16,
    device: Arc<Mutex<VirtualDevice>>,
) -> std::io::Result<()> {
    // Store active session: (IpAddr, Notify for connection reset)
    let active_session: Arc<Mutex<Option<(IpAddr, Arc<Notify>)>>> = Arc::new(Mutex::new(None));

    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
    let mut buf = [0u8; 64]; // Buffer larger than needed

    loop {
        let (len, src_addr) = socket.recv_from(&mut buf).await?;
        let src_ip = src_addr.ip();

        // Check if this IP is already connected
        let _is_new_client = {
            let mut session = active_session.lock().unwrap();
            if let Some((existing_ip, _)) = session.as_ref() {
                if *existing_ip == src_ip {
                    // Same client continuing: keep existing session
                    false
                } else {
                    // Different client: replace session
                    println!(
                        "UDP Gamepad connection from {} replacing previous connection from {}",
                        src_ip, existing_ip
                    );
                    let new_notify = Arc::new(Notify::new());
                    *session = Some((src_ip, new_notify));
                    true
                }
            } else {
                // First client
                println!("UDP Gamepad connection from {} registered", src_ip);
                let new_notify = Arc::new(Notify::new());
                *session = Some((src_ip, new_notify));
                true
            }
        };

        if len >= 29 && buf[0] == HEADER_GAMEPAD_SNAPSHOT {
            log_data(Verbosity::High, "UDP Gamepad Snapshot", &buf[..len]);

            // Parse buttons: 12 bytes starting from index 1
            let mut buttons = [0u8; 12];
            buttons.copy_from_slice(&buf[1..13]);

            // Parse axes: 8 i16 starting from index 13
            let mut axes = [0i16; 8];
            for i in 0..8 {
                let start = 13 + i * 2;
                axes[i] = i16::from_le_bytes([buf[start], buf[start + 1]]);
            }

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

    // Axis mappings
    let axis_types = [
        AbsoluteAxisType::ABS_X,    // Left X
        AbsoluteAxisType::ABS_Y,    // Left Y
        AbsoluteAxisType::ABS_RX,   // Right X
        AbsoluteAxisType::ABS_RY,   // Right Y
        AbsoluteAxisType::ABS_Z,    // Trigger L (but we'll treat as button)
        AbsoluteAxisType::ABS_RZ,   // Trigger R (but we'll treat as button)
        AbsoluteAxisType::ABS_HAT0X, // Hat X
        AbsoluteAxisType::ABS_HAT0Y, // Hat Y
    ];

    for (i, &value) in axes.iter().enumerate() {
        match i {
            0..=3 => { // Sticks: Left X/Y, Right X/Y
                if let Some(axis) = axis_types.get(i) {
                    events.push(InputEvent::new(EventType::ABSOLUTE, axis.0, value as i32));
                }
            }
            4 => { // Trigger L: treat as button BTN_THUMBL
                let val = if value == 0 { 0 } else { 1 };
                events.push(InputEvent::new(EventType::KEY, Key::BTN_THUMBL.0, val));
            }
            5 => { // Trigger R: treat as button BTN_THUMBR
                let val = if value == 0 { 0 } else { 1 };
                events.push(InputEvent::new(EventType::KEY, Key::BTN_THUMBR.0, val));
            }
            6..=7 => { // Hat X/Y: scale to -1/0/1
                if let Some(axis) = axis_types.get(i) {
                    let scaled = if value < 0 { -1 } else if value > 0 { 1 } else { 0 };
                    events.push(InputEvent::new(EventType::ABSOLUTE, axis.0, scaled));
                }
            }
            _ => {}
        }
    }

    if !events.is_empty() {
        if let Ok(mut dev) = device.lock() {
            let _ = dev.emit(&events);
        }
    }
}