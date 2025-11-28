mod devices;
mod discovery;
mod servers;
mod input_mode;
mod logger;
mod protocol;

use devices::{create_virtual_keyboard, create_virtual_mouse};
use discovery::run_discovery_broadcast;
use devices::create_virtual_gamepad;
use servers::gamepad_server::run_udp_gamepad_server;
use input_mode::InputMode;
use servers::keyboard_server::run_tcp_keyboard_server;
use logger::{log, set_verbosity, Verbosity};
use servers::mouse_server::run_udp_mouse_server;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

const UDP_PORT: u16 = 5555;
const TCP_PORT: u16 = 5556;
const GAMEPAD_UDP_PORT: u16 = 5558;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let verbosity = if args.len() > 1 && args[1] == "--verbosity" && args.len() > 2 {
        args[2].parse::<u8>().unwrap_or(0)
    } else {
        0
    };
    set_verbosity(Verbosity::from_u8(verbosity));

    log(Verbosity::Low, "🚀 Iniciando Retro Control Server...");

    let mouse = Arc::new(Mutex::new(create_virtual_mouse()?));
    let keyboard = Arc::new(Mutex::new(create_virtual_keyboard()?));
    let gamepad = Arc::new(Mutex::new(create_virtual_gamepad()?));
    let input_mode = Arc::new(RwLock::new(InputMode::MouseKeyboard));

    println!("✓ Dispositivos virtuales creados");

    let connected_clients = Arc::new(AtomicUsize::new(0));
    let mouse_clone = mouse.clone();
    tokio::spawn(async move {
        if let Err(e) = run_udp_mouse_server(UDP_PORT, mouse_clone).await {
            log(Verbosity::Low, &format!("Error en servidor UDP Mouse: {}", e));
        }
    });

    let keyboard_clone = keyboard.clone();
    let mode_clone = input_mode.clone();
    let tcp_clients_clone = connected_clients.clone();
    tokio::spawn(async move {
        if let Err(e) = run_tcp_keyboard_server(
            TCP_PORT,
            keyboard_clone,
            mode_clone,
            tcp_clients_clone,
        )
        .await
        {
            log(Verbosity::Low, &format!("Error en servidor TCP Teclado: {}", e));
        }
    });

    let gamepad_clone = gamepad.clone();
    tokio::spawn(async move {
        if let Err(e) = run_udp_gamepad_server(GAMEPAD_UDP_PORT, gamepad_clone).await {
            log(Verbosity::Low, &format!("Error en servidor UDP Gamepad: {}", e));
        }
    });

    let discovery_clients = connected_clients.clone();
    tokio::spawn(async move {
        if let Err(e) = run_discovery_broadcast(TCP_PORT, UDP_PORT, discovery_clients).await {
            log(Verbosity::Low, &format!("Error en broadcast de descubrimiento: {}", e));
        }
    });

    log(Verbosity::Low, "✓ Servidores de red iniciados");
    log(Verbosity::Low, &format!("   - Mouse UDP: 0.0.0.0:{}", UDP_PORT));
    log(Verbosity::Low, &format!("   - Teclado TCP: 0.0.0.0:{}", TCP_PORT));
    log(Verbosity::Low, &format!("   - Gamepad UDP: 0.0.0.0:{}", GAMEPAD_UDP_PORT));
    log(Verbosity::Low, "Esperando conexiones...");

    tokio::signal::ctrl_c().await?;
    log(Verbosity::Low, "\nApagando Retro Control Server...");

    Ok(())
}