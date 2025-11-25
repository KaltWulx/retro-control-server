mod discovery;
mod devices;
mod gamepad_device;
mod input_mode;
mod keyboard_server;
mod mouse_server;
mod protocol;

use devices::{create_virtual_keyboard, create_virtual_mouse};
use gamepad_device::create_virtual_gamepad;
use input_mode::InputMode;
use keyboard_server::run_tcp_keyboard_server;
use mouse_server::run_udp_mouse_server;
use discovery::run_discovery_broadcast;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicUsize;
use tokio::sync::RwLock;

const UDP_PORT: u16 = 5555;
const TCP_PORT: u16 = 5556;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Iniciando Retro Control Server...");

    let mouse = Arc::new(Mutex::new(create_virtual_mouse()?));
    let keyboard = Arc::new(Mutex::new(create_virtual_keyboard()?));
    let gamepad = Arc::new(Mutex::new(create_virtual_gamepad()?));
    let input_mode = Arc::new(RwLock::new(InputMode::MouseKeyboard));

    println!("✓ Dispositivos virtuales creados");

    let connected_clients = Arc::new(AtomicUsize::new(0));

    let mouse_clone = mouse.clone();
    tokio::spawn(async move {
        if let Err(e) = run_udp_mouse_server(UDP_PORT, mouse_clone).await {
            eprintln!("Error en servidor UDP Mouse: {}", e);
        }
    });

    let keyboard_clone = keyboard.clone();
    let mode_clone = input_mode.clone();
    let tcp_clients_clone = connected_clients.clone();
    tokio::spawn(async move {
        if let Err(e) = run_tcp_keyboard_server(
            TCP_PORT,
            keyboard_clone,
            gamepad.clone(),
            mode_clone,
            tcp_clients_clone,
        )
        .await
        {
            eprintln!("Error en servidor TCP Teclado: {}", e);
        }
    });

    let discovery_clients = connected_clients.clone();
    tokio::spawn(async move {
        if let Err(e) = run_discovery_broadcast(TCP_PORT, UDP_PORT, discovery_clients).await {
            eprintln!("Error en broadcast de descubrimiento: {}", e);
        }
    });

    println!("✓ Servidores de red iniciados");
    println!("   - Mouse UDP: 0.0.0.0:{}", UDP_PORT);
    println!("   - Teclado TCP: 0.0.0.0:{}", TCP_PORT);
    println!("Esperando conexiones...");

    tokio::signal::ctrl_c().await?;
    println!("\nApagando Retro Control Server...");

    Ok(())
}
