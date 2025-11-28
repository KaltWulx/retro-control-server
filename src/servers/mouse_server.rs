use crate::logger::{log, log_data, Verbosity};
use crate::protocol::HEADER_MOUSE;
use evdev::{EventType, InputEvent, Key, RelativeAxisType, uinput::VirtualDevice};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tokio::sync::Notify;

const BTN_MASK_LEFT: u8 = 0x01;
const BTN_MASK_RIGHT: u8 = 0x02;
const BTN_MASK_MIDDLE: u8 = 0x04;

pub async fn run_udp_mouse_server(
    port: u16,
    device: Arc<Mutex<VirtualDevice>>,
) -> std::io::Result<()> {
    // Store active session: (IpAddr, Notify for connection reset)
    let active_session: Arc<Mutex<Option<(IpAddr, Arc<Notify>)>>> = Arc::new(Mutex::new(None));

    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
    let mut buf = [0u8; 32];
    let mut last_buttons = 0u8;

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
                        "UDP connection from {} replacing previous connection from {}",
                        src_ip, existing_ip
                    );
                    let new_notify = Arc::new(Notify::new());
                    *session = Some((src_ip, new_notify));
                    true
                }
            } else {
                // First client
                println!("UDP connection from {} registered", src_ip);
                let new_notify = Arc::new(Notify::new());
                *session = Some((src_ip, new_notify));
                true
            }
        };

        if len >= 7 && buf[0] == HEADER_MOUSE {
            log_data(Verbosity::High, "UDP Mouse Packet", &buf[..len]);
            let dx = i16::from_le_bytes([buf[1], buf[2]]);
            let dy = i16::from_le_bytes([buf[3], buf[4]]);
            let buttons = buf[5];

            let wheel = if len >= 8 {
                i16::from_le_bytes([buf[6], buf[7]])
            } else {
                0
            };

            log(Verbosity::High, &format!("Mouse: dx={}, dy={}, buttons={:02X}, wheel={}", dx, dy, buttons, wheel));

            let mut events = Vec::with_capacity(6);

            if dx != 0 {
                events.push(InputEvent::new(
                    EventType::RELATIVE,
                    RelativeAxisType::REL_X.0,
                    dx as i32,
                ));
            }
            if dy != 0 {
                events.push(InputEvent::new(
                    EventType::RELATIVE,
                    RelativeAxisType::REL_Y.0,
                    dy as i32,
                ));
            }
            if wheel != 0 {
                events.push(InputEvent::new(
                    EventType::RELATIVE,
                    RelativeAxisType::REL_WHEEL.0,
                    wheel as i32,
                ));
            }

            let changed = buttons ^ last_buttons;

            if changed & BTN_MASK_LEFT != 0 {
                let val = if buttons & BTN_MASK_LEFT != 0 { 1 } else { 0 };
                events.push(InputEvent::new(EventType::KEY, Key::BTN_LEFT.0, val));
            }
            if changed & BTN_MASK_RIGHT != 0 {
                let val = if buttons & BTN_MASK_RIGHT != 0 { 1 } else { 0 };
                events.push(InputEvent::new(EventType::KEY, Key::BTN_RIGHT.0, val));
            }
            if changed & BTN_MASK_MIDDLE != 0 {
                let val = if buttons & BTN_MASK_MIDDLE != 0 { 1 } else { 0 };
                events.push(InputEvent::new(EventType::KEY, Key::BTN_MIDDLE.0, val));
            }

            last_buttons = buttons;

            if !events.is_empty() {
                if let Ok(mut dev) = device.lock() {
                    let _ = dev.emit(&events);
                }
            }
        }
    }
}
