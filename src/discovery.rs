use crate::protocol::{DISCOVERY_INTERVAL_MS, DISCOVERY_PORT, HEADER_DISCOVERY};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::UdpSocket;
use tokio::time::{Duration, sleep};

pub async fn run_discovery_broadcast(
    tcp_port: u16,
    udp_port: u16,
    active_clients: Arc<AtomicUsize>,
) -> std::io::Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;
    socket.set_broadcast(true)?;

    let mut payload = [0u8; 5];
    loop {
        if active_clients.load(Ordering::SeqCst) == 0 {
            payload[0] = HEADER_DISCOVERY;
            payload[1..3].copy_from_slice(&tcp_port.to_le_bytes());
            payload[3..5].copy_from_slice(&udp_port.to_le_bytes());
            match socket
                .send_to(&payload, ("255.255.255.255", DISCOVERY_PORT))
                .await
            {
                Ok(size) => {
                    let clients = active_clients.load(Ordering::SeqCst);
                    println!(
                        "Sent discovery packet ({} bytes) TCP:{} UDP:{} active_clients:{}",
                        size, tcp_port, udp_port, clients
                    );
                }
                Err(e) => {
                    eprintln!("Error broadcasting discovery packet: {}", e);
                }
            }
        }
        sleep(Duration::from_millis(DISCOVERY_INTERVAL_MS)).await;
    }
}
