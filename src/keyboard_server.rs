use crate::input_mode::InputMode;
use crate::logger::{log_block, log_detail, Verbosity};
use crate::protocol::{
    GAMEPAD_AXIS_HAT_X, GAMEPAD_AXIS_HAT_Y, GAMEPAD_AXIS_LEFT_X, GAMEPAD_AXIS_LEFT_Y,
    GAMEPAD_AXIS_RIGHT_X, GAMEPAD_AXIS_RIGHT_Y, GAMEPAD_AXIS_TRIGGER_L, GAMEPAD_AXIS_TRIGGER_R,
    GAMEPAD_BUTTON_A, GAMEPAD_BUTTON_B, GAMEPAD_BUTTON_BACK, GAMEPAD_BUTTON_GUIDE, GAMEPAD_BUTTON_HOTKEY,
    GAMEPAD_BUTTON_LB, GAMEPAD_BUTTON_RB, GAMEPAD_BUTTON_START, GAMEPAD_BUTTON_THUMB_L,
    GAMEPAD_BUTTON_THUMB_R, GAMEPAD_BUTTON_X, GAMEPAD_BUTTON_Y, HEADER_GAMEPAD_AXIS,
    HEADER_GAMEPAD_BUTTON, HEADER_KEYBOARD, HEADER_MODE_ACK, HEADER_MODE_SWITCH,
};
use evdev::{AbsoluteAxisType, EventType, InputEvent, Key, uinput::VirtualDevice};
use std::io::ErrorKind;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, RwLock};

pub async fn run_tcp_keyboard_server(
    port: u16,
    device: Arc<Mutex<VirtualDevice>>,
    gamepad: Arc<Mutex<VirtualDevice>>,
    input_mode: Arc<RwLock<InputMode>>,
    active_clients: Arc<AtomicUsize>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    let active_session: Arc<Mutex<Option<(IpAddr, u64, Arc<Notify>)>>> = Arc::new(Mutex::new(None));
    let connection_id_counter = Arc::new(AtomicU64::new(0));

    loop {
        let (socket, addr) = listener.accept().await?;
        let peer_ip = addr.ip();
        log_detail(Verbosity::Medium, "Conexión TCP aceptada", &format!("ip={}", peer_ip));
        let connection_id = connection_id_counter.fetch_add(1, Ordering::SeqCst);

        let old_notifier = {
            let session = active_session.lock().unwrap();
            if let Some((existing_ip, _, old_notify)) = session.as_ref() {
                if *existing_ip == peer_ip {
                    log_detail(Verbosity::Low, "Conexión TCP existente", &format!("cerrando ip={}", peer_ip));
                    Some(old_notify.clone())
                } else {
                    log_detail(Verbosity::Low, "Conexión TCP rechazada", &format!("ip={} ya ligada a {}", peer_ip, existing_ip));
                    continue;
                }
            } else {
                None
            }
        };

        if let Some(notifier) = old_notifier {
            notifier.notify_one();
        }

        let new_notify = Arc::new(Notify::new());
        {
            let mut session = active_session.lock().unwrap();
            *session = Some((peer_ip, connection_id, new_notify.clone()));
        }

        log_detail(Verbosity::Low, "Conexión TCP registrada", &format!("ip={}", peer_ip));

        let dev_clone = device.clone();
        let gamepad_clone = gamepad.clone();
        let mode_clone = input_mode.clone();
        let session_clone = active_session.clone();
        let cancel_signal = new_notify.clone();
        let connection_id_clone = connection_id;
        let client_counter = active_clients.clone();

        tokio::spawn(async move {
            let _guard = ConnectionGuard::new(client_counter);

            tokio::select! {
                result = handle_tcp_client(socket, dev_clone, gamepad_clone, mode_clone) => {
                    if let Err(e) = result {
                        log_detail(Verbosity::Low, "Error en conexión TCP", &format!("{}: {}", addr, e));
                    }
                }
                _ = cancel_signal.notified() => {
                    log_detail(Verbosity::Low, "Conexión TCP terminada", &format!("ip={} por nueva conexión", peer_ip));
                }
            }

            let should_clear_session = {
                let mut session = session_clone.lock().unwrap();
                if let Some((active_ip, active_id, _)) = session.as_ref() {
                    if *active_ip == peer_ip && *active_id == connection_id_clone {
                        *session = None;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            if should_clear_session {
                log_detail(Verbosity::Low, "Conexión TCP removida", &format!("ip={} contador reseteado", peer_ip));
            }
        });
    }
}

async fn handle_tcp_client(
    mut socket: TcpStream,
    device: Arc<Mutex<VirtualDevice>>,
    gamepad: Arc<Mutex<VirtualDevice>>,
    input_mode: Arc<RwLock<InputMode>>,
) -> std::io::Result<()> {
    fn is_connection_closed(err: &std::io::Error) -> bool {
        matches!(
            err.kind(),
            ErrorKind::UnexpectedEof
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::BrokenPipe
        )
    }

    let mut header = [0u8; 1];

    loop {
        if let Err(e) = socket.read_exact(&mut header).await {
            if is_connection_closed(&e) {
                break;
            }
            return Err(e);
        }

        match header[0] {
            HEADER_MODE_SWITCH => {
                let mut mode_byte = [0u8; 1];
                if let Err(e) = socket.read_exact(&mut mode_byte).await {
                    if is_connection_closed(&e) {
                        break;
                    }
                    return Err(e);
                }
                log_block("TCP Packet", vec![
                    format!("type=Mode Switch"),
                    format!("header={:02X}", header[0]),
                    format!("mode={}", mode_byte[0]),
                    format!("raw={:02X}", mode_byte[0])
                ]);

                if let Some(new_mode) = InputMode::from_byte(mode_byte[0]) {
                    {
                        let mut guard = input_mode.write().await;
                        if *guard != new_mode {
                            match new_mode {
                                InputMode::Gamepad => {
                                    log_detail(Verbosity::Low, "Modo cambiado", "a gamepad");
                                }
                                InputMode::MouseKeyboard => {
                                    log_detail(Verbosity::Low, "Modo cambiado", "a mouse+teclado");
                                }
                            }
                        }
                        *guard = new_mode;
                    }
                    socket.write_all(&[HEADER_MODE_ACK, mode_byte[0]]).await?;
                } else {
                    socket.write_all(&[HEADER_MODE_ACK, 0xFF]).await?;
                }
            }
            HEADER_KEYBOARD => {
                let mut payload = [0u8; 2];
                if let Err(e) = socket.read_exact(&mut payload).await {
                    if is_connection_closed(&e) {
                        break;
                    }
                    return Err(e);
                }
                log_block("TCP Packet", vec![
                    format!("type=Keyboard"),
                    format!("header={:02X}", header[0]),
                    format!("scancode={}", payload[0]),
                    format!("state={}", payload[1]),
                    format!("raw={:02X} {:02X}", payload[0], payload[1])
                ]);

                if *input_mode.read().await == InputMode::MouseKeyboard {
                    process_keyboard_event(payload[0], payload[1], &device);
                }
            }
            HEADER_GAMEPAD_AXIS => {
                let mut payload = [0u8; 3];
                if let Err(e) = socket.read_exact(&mut payload).await {
                    if is_connection_closed(&e) {
                        break;
                    }
                    return Err(e);
                }
                let value = i16::from_le_bytes([payload[1], payload[2]]);
                log_block("Gamepad Axis", vec![
                    format!("header={:02X}", header[0]),
                    format!("axis_id={} ({})", payload[0], axis_name(payload[0])),
                    format!("raw={:02X} {:02X} {:02X}", payload[0], payload[1], payload[2]),
                    format!("value={}", value)
                ]);

                if *input_mode.read().await == InputMode::Gamepad {
                    let axis_id = payload[0];
                    let value = i16::from_le_bytes([payload[1], payload[2]]);
                    emit_gamepad_axis(axis_id, value, &gamepad);
                }
            }
            HEADER_GAMEPAD_BUTTON => {
                let mut payload = [0u8; 2];
                if let Err(e) = socket.read_exact(&mut payload).await {
                    if is_connection_closed(&e) {
                        break;
                    }
                    return Err(e);
                }
                log_block("Gamepad Button", vec![
                    format!("header={:02X}", header[0]),
                    format!("button_id={} ({})", payload[0], button_name(payload[0])),
                    format!("state={} ({})", payload[1], if payload[1] > 0 { "pressed" } else { "released" }),
                    format!("raw={:02X} {:02X}", payload[0], payload[1])
                ]);

                if *input_mode.read().await == InputMode::Gamepad {
                    let button_id = payload[0];
                    let state = payload[1];
                    emit_gamepad_button(button_id, state, &gamepad);
                }
            }
            other => {
                log_block("TCP Packet", vec![
                    format!("type=Unknown"),
                    format!("header={:02X}", other)
                ]);
            }
        }
    }

    Ok(())
}

fn process_keyboard_event(scancode: u8, state: u8, device: &Arc<Mutex<VirtualDevice>>) {
    let key_code = map_keyboard_key(scancode);
    let key = Key::new(key_code);
    let val = if state > 0 { 1 } else { 0 };
    let event = InputEvent::new(evdev::EventType::KEY, key.0, val);

    if let Ok(mut dev) = device.lock() {
        let _ = dev.emit(&[event]);
    }
}

fn emit_gamepad_axis(axis_id: u8, value: i16, device: &Arc<Mutex<VirtualDevice>>) {
    match axis_id {
        GAMEPAD_AXIS_TRIGGER_L => {
            let key = Key::BTN_THUMBL;
            let val = if value == 0 { 0 } else { 1 };
            let event = InputEvent::new(EventType::KEY, key.0, val);
            if let Ok(mut dev) = device.lock() {
                let _ = dev.emit(&[event]);
            }
        }
        GAMEPAD_AXIS_TRIGGER_R => {
            let key = Key::BTN_THUMBR;
            let val = if value == 0 { 0 } else { 1 };
            let event = InputEvent::new(EventType::KEY, key.0, val);
            if let Ok(mut dev) = device.lock() {
                let _ = dev.emit(&[event]);
            }
        }
        GAMEPAD_AXIS_HAT_X | GAMEPAD_AXIS_HAT_Y => {
            if let Some(axis) = map_axis(axis_id) {
                let scaled_value = if value < 0 { -1 } else if value > 0 { 1 } else { 0 };
                let event = InputEvent::new(EventType::ABSOLUTE, axis.0, scaled_value);
                if let Ok(mut dev) = device.lock() {
                    let _ = dev.emit(&[event]);
                }
            }
        }
        _ => {
            if let Some(axis) = map_axis(axis_id) {
                let event = InputEvent::new(EventType::ABSOLUTE, axis.0, value as i32);
                if let Ok(mut dev) = device.lock() {
                    let _ = dev.emit(&[event]);
                }
            }
        }
    }
}

fn emit_gamepad_button(button_id: u8, state: u8, device: &Arc<Mutex<VirtualDevice>>) {
    if let Some(key) = map_button(button_id) {
        let val = if state > 0 { 1 } else { 0 };
        let event = InputEvent::new(EventType::KEY, key.0, val);
        if let Ok(mut dev) = device.lock() {
            let _ = dev.emit(&[event]);
        }
    }
}

fn map_axis(axis_id: u8) -> Option<AbsoluteAxisType> {
    match axis_id {
        GAMEPAD_AXIS_LEFT_X => Some(AbsoluteAxisType::ABS_X),
        GAMEPAD_AXIS_LEFT_Y => Some(AbsoluteAxisType::ABS_Y),
        GAMEPAD_AXIS_RIGHT_X => Some(AbsoluteAxisType::ABS_RX),
        GAMEPAD_AXIS_RIGHT_Y => Some(AbsoluteAxisType::ABS_RY),
        GAMEPAD_AXIS_TRIGGER_L => Some(AbsoluteAxisType::ABS_Z),
        GAMEPAD_AXIS_TRIGGER_R => Some(AbsoluteAxisType::ABS_RZ),
        GAMEPAD_AXIS_HAT_X => Some(AbsoluteAxisType::ABS_HAT0X),
        GAMEPAD_AXIS_HAT_Y => Some(AbsoluteAxisType::ABS_HAT0Y),
        _ => None,
    }
}

fn map_button(button_id: u8) -> Option<Key> {
    match button_id {
        GAMEPAD_BUTTON_A => Some(Key::BTN_SOUTH),
        GAMEPAD_BUTTON_B => Some(Key::BTN_EAST),
        GAMEPAD_BUTTON_X => Some(Key::BTN_NORTH),
        GAMEPAD_BUTTON_Y => Some(Key::BTN_WEST),
        GAMEPAD_BUTTON_LB => Some(Key::BTN_TL),
        GAMEPAD_BUTTON_RB => Some(Key::BTN_TR),
        GAMEPAD_BUTTON_START => Some(Key::BTN_START),
        GAMEPAD_BUTTON_BACK => Some(Key::BTN_SELECT),
        GAMEPAD_BUTTON_THUMB_L => Some(Key::BTN_THUMBL),
        GAMEPAD_BUTTON_THUMB_R => Some(Key::BTN_THUMBR),
        GAMEPAD_BUTTON_HOTKEY => Some(Key::BTN_MODE),
        GAMEPAD_BUTTON_GUIDE => Some(Key::BTN_MODE),
        _ => None,
    }
}

fn map_keyboard_key(scancode: u8) -> u16 {
    match scancode {
        // Fix for Android clients sending Android Keycodes for some keys
        // Android KEYCODE_MINUS (69) -> Linux KEY_MINUS (12)
        69 => 12,
        // Android KEYCODE_EQUALS (70) -> Linux KEY_EQUAL (13)
        70 => 13,
        // Android KEYCODE_PLUS (81) -> Linux KEY_KPPLUS (78)
        81 => 78,
        // Pass through others (assuming they are already Linux evdev codes)
        c => c as u16,
    }
}

struct ConnectionGuard {
    counter: Arc<AtomicUsize>,
}

impl ConnectionGuard {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self { counter }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

fn button_name(id: u8) -> &'static str {
    match id {
        GAMEPAD_BUTTON_A => "A",
        GAMEPAD_BUTTON_B => "B",
        GAMEPAD_BUTTON_X => "X",
        GAMEPAD_BUTTON_Y => "Y",
        GAMEPAD_BUTTON_LB => "LB",
        GAMEPAD_BUTTON_RB => "RB",
        GAMEPAD_BUTTON_START => "Start",
        GAMEPAD_BUTTON_BACK => "Back",
        GAMEPAD_BUTTON_THUMB_L => "Thumb L",
        GAMEPAD_BUTTON_THUMB_R => "Thumb R",
        GAMEPAD_BUTTON_HOTKEY => "Hotkey",
        GAMEPAD_BUTTON_GUIDE => "Guide",
        _ => "Unknown",
    }
}

fn axis_name(id: u8) -> &'static str {
    match id {
        GAMEPAD_AXIS_LEFT_X => "Left X",
        GAMEPAD_AXIS_LEFT_Y => "Left Y",
        GAMEPAD_AXIS_RIGHT_X => "Right X",
        GAMEPAD_AXIS_RIGHT_Y => "Right Y",
        GAMEPAD_AXIS_TRIGGER_L => "Trigger L",
        GAMEPAD_AXIS_TRIGGER_R => "Trigger R",
        GAMEPAD_AXIS_HAT_X => "Hat X",
        GAMEPAD_AXIS_HAT_Y => "Hat Y",
        _ => "Unknown",
    }
}
