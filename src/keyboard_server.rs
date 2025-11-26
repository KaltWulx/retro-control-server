use crate::input_mode::InputMode;
use crate::protocol::{
    GAMEPAD_AXIS_HAT_X, GAMEPAD_AXIS_HAT_Y, GAMEPAD_AXIS_LEFT_X, GAMEPAD_AXIS_LEFT_Y,
    GAMEPAD_AXIS_RIGHT_X, GAMEPAD_AXIS_RIGHT_Y, GAMEPAD_AXIS_TRIGGER_L, GAMEPAD_AXIS_TRIGGER_R,
    GAMEPAD_BUTTON_A, GAMEPAD_BUTTON_B, GAMEPAD_BUTTON_BACK, GAMEPAD_BUTTON_HOTKEY,
    GAMEPAD_BUTTON_LB, GAMEPAD_BUTTON_RB, GAMEPAD_BUTTON_START, GAMEPAD_BUTTON_THUMB_L,
    GAMEPAD_BUTTON_THUMB_R, GAMEPAD_BUTTON_X, GAMEPAD_BUTTON_Y, HEADER_GAMEPAD_AXIS,
    HEADER_GAMEPAD_BUTTON, HEADER_KEYBOARD, HEADER_MODE_ACK, HEADER_MODE_SWITCH,
};
use evdev::{AbsoluteAxisType, EventType, InputEvent, Key, uinput::VirtualDevice};
use std::io::ErrorKind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

pub async fn run_tcp_keyboard_server(
    port: u16,
    device: Arc<Mutex<VirtualDevice>>,
    gamepad: Arc<Mutex<VirtualDevice>>,
    input_mode: Arc<RwLock<InputMode>>,
    active_clients: Arc<AtomicUsize>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("Nueva conexión TCP desde: {}", addr);

        let dev_clone = device.clone();
        let gamepad_clone = gamepad.clone();
        let mode_clone = input_mode.clone();
        let client_counter = active_clients.clone();

        tokio::spawn(async move {
            let _guard = ConnectionGuard::new(client_counter);
            if let Err(e) = handle_tcp_client(socket, dev_clone, gamepad_clone, mode_clone).await {
                eprintln!("Error en conexión TCP {}: {}", addr, e);
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

                if let Some(new_mode) = InputMode::from_byte(mode_byte[0]) {
                    {
                        let mut guard = input_mode.write().await;
                        if *guard != new_mode {
                            match new_mode {
                                InputMode::Gamepad => {
                                    println!(
                                        "Modo cambiado a gamepad; ignorando paquetes de teclado hasta cambiar de modo"
                                    );
                                }
                                InputMode::MouseKeyboard => {
                                    println!(
                                        "Modo cambiado a mouse+teclado; procesando paquetes de teclado"
                                    );
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

                if *input_mode.read().await == InputMode::Gamepad {
                    let axis_id = payload[0];
                    let value = i16::from_le_bytes([payload[1], payload[2]]);
                    println!("Gamepad axis received: id={} value={}", axis_id, value);
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

                if *input_mode.read().await == InputMode::Gamepad {
                    let button_id = payload[0];
                    let state = payload[1];
                    println!("Gamepad button received: id={} state={}", button_id, state);
                    emit_gamepad_button(button_id, state, &gamepad);
                }
            }
            other => {
                println!("Paquete TCP desconocido {:02X}; descartado", other);
            }
        }
    }

    Ok(())
}

fn process_keyboard_event(scancode: u8, state: u8, device: &Arc<Mutex<VirtualDevice>>) {
    let key = Key::new(scancode as u16);
    let val = if state > 0 { 1 } else { 0 };
    let event = InputEvent::new(evdev::EventType::KEY, key.0, val);

    if let Ok(mut dev) = device.lock() {
        let _ = dev.emit(&[event]);
    }
}

fn emit_gamepad_axis(axis_id: u8, value: i16, device: &Arc<Mutex<VirtualDevice>>) {
    if let Some(axis) = map_axis(axis_id) {
        let event = InputEvent::new(EventType::ABSOLUTE, axis.0, value as i32);
        if let Ok(mut dev) = device.lock() {
            let _ = dev.emit(&[event]);
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
        _ => None,
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
