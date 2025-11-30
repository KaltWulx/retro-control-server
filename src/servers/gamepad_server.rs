use crate::logger::{log, log_data, Verbosity};
use crate::protocol::HEADER_GAMEPAD_SNAPSHOT;
use crate::devices::xbox360_layout::Xbox360Layout;
use evdev::{EventType, InputEvent, Key, uinput::VirtualDevice};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::net::UdpSocket;

// Mode detection constants
const MODE_ARCADE: u8 = 1;   // Arcade layout (snap to 8 directions + -32768)
const MODE_XBOX: u8 = 2;     // Xbox layout with real intermediate values

// Global variable to remember the detected mode
static CURRENT_MODE: AtomicU8 = AtomicU8::new(0); // 0 = not detected yet

pub async fn run_udp_gamepad_server(
    port: u16,
    device: Arc<Mutex<VirtualDevice>>,
) -> std::io::Result<()> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
    let mut buf = [0u8; 64];

    loop {
        let (len, _) = socket.recv_from(&mut buf).await?;
        let data = buf[..len].to_vec();
        let device_clone = Arc::clone(&device);

        // Spawn processing to keep recv loop fast
        tokio::spawn(async move {
            if let Some((mode, buttons, axes)) = parse_gamepad_snapshot(&data) {
                log(Verbosity::Low, &format!("Gamepad Snapshot: mode={}, buttons={:?}, axes={:?}", mode, buttons, axes));
                let semantic = describe_snapshot(&buttons, &axes);
                log_data(Verbosity::Low, &format!("Evento: {}", semantic), &[]);

                let mut events = Vec::new();
                process_buttons(buttons, &mut events);
                process_axes(mode, axes, &mut events);
                emit_events(&device_clone, &events);
            }
        });
    }
}

fn parse_gamepad_snapshot(buf: &[u8]) -> Option<(u8, [u8; 12], [i16; 8])> {
    // Formato: [header:1][mode:1][button_bits:2][axes:16]
    if buf.len() >= 20 && buf[0] == HEADER_GAMEPAD_SNAPSHOT {
        log_data(Verbosity::Low, "UDP Gamepad Snapshot", buf);

        let mode = buf[1];

        // Botones: bitwise en 2 bytes (u16 LE)
        let button_bits = u16::from_le_bytes([buf[2], buf[3]]);
        let mut buttons = [0u8; 12];
        for i in 0..12 {
            buttons[i] = ((button_bits >> i) & 1) as u8;
        }

        // Ejes: 8 x i16 LE
        let mut axes = [0i16; 8];
        for i in 0..8 {
            let start = 4 + i * 2;
            axes[i] = i16::from_le_bytes([buf[start], buf[start + 1]]);
        }

        Some((mode, buttons, axes))
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

fn process_axes(_mode: u8, axes: [i16; 8], events: &mut Vec<InputEvent>) {
    // ------------------------------------------------------------------
    // 1. Automatic mode detection (only the first time)
    // ------------------------------------------------------------------
    let current = CURRENT_MODE.load(Ordering::Relaxed);
    if current == 0 {
        // If we ever see -32768 → almost certainly arcade mode
        // If we see values like -16384, 12000, etc. → xbox analog mode
        let is_arcade = axes[0] == -32768 || axes[0] == 32767 || 
                        axes[1] == -32768 || axes[1] == 32767 ||
                        axes[0] == -32767 || axes[1] == 32767; // old compatibility

        let detected_mode = if is_arcade { MODE_ARCADE } else { MODE_XBOX };
        CURRENT_MODE.store(detected_mode, Ordering::Relaxed);
        log(Verbosity::Low, &format!("Modo de gamepad detectado: {}", 
            if detected_mode == MODE_ARCADE { "ARCADE (8 direcciones)" } else { "XBOX (analógico)" }));
    }

    let detected_mode = CURRENT_MODE.load(Ordering::Relaxed);

    // ------------------------------------------------------------------
    // 2. Processing based on detected mode
    // ------------------------------------------------------------------
    if detected_mode == MODE_ARCADE {
        // ===== ARCADE MODE (perfect logs for combos) =====
        // Left stick → ABS_X / ABS_Y (analog, needed for some cores)
        emit_axis(events, 0x00, axes[0] as i32); // ABS_X
        emit_axis(events, 0x01, axes[1] as i32); // ABS_Y

        // Left stick → DIGITAL D-PAD (ABS_HAT0X/HAT0Y) → this is what 95% of retro games read
        let hat_x = if axes[0] <= -20000 { -1 } else if axes[0] >= 20000 { 1 } else { 0 };
        let hat_y = if axes[1] <= -20000 { -1 } else if axes[1] >= 20000 { 1 } else { 0 };
        emit_axis(events, 0x10, hat_x); // ABS_HAT0X
        emit_axis(events, 0x11, hat_y); // ABS_HAT0Y

        // Right stick (if used)
        emit_axis(events, 0x03, axes[2] as i32); // ABS_RX
        emit_axis(events, 0x04, axes[3] as i32); // ABS_RY

        // Triggers
        emit_axis(events, 0x02, axes[4] as i32); // ABS_Z (L trigger)
        emit_axis(events, 0x05, axes[5] as i32); // ABS_RZ (R trigger)

        // D-pad axes (indices 6, 7) - scale to -1/0/1
        let dpad_x = if axes[6] < 0 { -1 } else if axes[6] > 0 { 1 } else { 0 };
        let dpad_y = if axes[7] < 0 { -1 } else if axes[7] > 0 { 1 } else { 0 };
        emit_axis(events, 0x10, dpad_x); // ABS_HAT0X (may override, but that's ok)
        emit_axis(events, 0x11, dpad_y); // ABS_HAT0Y (may override, but that's ok)
    } 
    else {
        // ===== CLASSIC XBOX 360 MODE (intermediate values) =====
        // Only emit normal analog axes (original code)
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
        }
    }
}

// Helper function to reduce code duplication
fn emit_axis(events: &mut Vec<InputEvent>, code: u16, value: i32) {
    events.push(InputEvent::new(EventType::ABSOLUTE, code, value));
}

fn emit_events(device: &Arc<Mutex<VirtualDevice>>, events: &[InputEvent]) {
    if !events.is_empty() {
        if let Ok(mut dev) = device.lock() {
            let _ = dev.emit(events);
            // ¡¡ESTO ES CRÍTICO EN BATOCERA!!
            let _ = dev.emit(&[InputEvent::new(EventType::SYNCHRONIZATION, 1, 0)]); // SYN_REPORT
        }
    }
}


fn describe_snapshot(buttons: &[u8; 12], axes: &[i16; 8]) -> String {
    let button_names = [
        "A", "B", "X", "Y", "LB", "RB", "Back", "Start", "Guide", "L3", "R3", "(unused)"
    ];
    let mut desc = Vec::new();
    for (i, &val) in buttons.iter().enumerate() {
        if val != 0 {
            desc.push(format!("BTN.{}", button_names[i]));
        }
    }
    // Ejes principales
    if axes[0] > 0 {
        desc.push("joystick_left derecha".to_string());
    } else if axes[0] < 0 {
        desc.push("joystick_left izquierda".to_string());
    }
    if axes[1] > 0 {
        desc.push("joystick_left abajo".to_string());
    } else if axes[1] < 0 {
        desc.push("joystick_left arriba".to_string());
    }
    if axes[2] > 0 {
        desc.push("joystick_right derecha".to_string());
    } else if axes[2] < 0 {
        desc.push("joystick_right izquierda".to_string());
    }
    if axes[3] > 0 {
        desc.push("joystick_right abajo".to_string());
    } else if axes[3] < 0 {
        desc.push("joystick_right arriba".to_string());
    }
    if axes[4] > 0 {
        desc.push(format!("trigger_left {}", axes[4]));
    }
    if axes[5] > 0 {
        desc.push(format!("trigger_right {}", axes[5]));
    }
    if axes[6] > 0 {
        desc.push("dpad derecha".to_string());
    } else if axes[6] < 0 {
        desc.push("dpad izquierda".to_string());
    }
    if axes[7] > 0 {
        desc.push("dpad abajo".to_string());
    } else if axes[7] < 0 {
        desc.push("dpad arriba".to_string());
    }
    if desc.is_empty() {
        "(sin acción)".to_string()
    } else {
        desc.join(", ")
    }
}