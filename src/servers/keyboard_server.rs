use crate::input_mode::InputMode;
use crate::logger::{log_block, log_detail, Verbosity};
use crate::protocol::{
    HEADER_KEYBOARD, HEADER_MODE_ACK, HEADER_MODE_SWITCH,
};
use evdev::{InputEvent, Key, uinput::VirtualDevice};
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
        let mode_clone = input_mode.clone();
        let session_clone = active_session.clone();
        let cancel_signal = new_notify.clone();
        let connection_id_clone = connection_id;
        let client_counter = active_clients.clone();

        tokio::spawn(async move {
            let _guard = ConnectionGuard::new(client_counter);

            tokio::select! {
                result = handle_tcp_client(socket, dev_clone, mode_clone) => {
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
