// main.rs

use client::accept_client_connections;
use common::proquint::Proquint;
use common::utils::{log_env, wait_terminate};
use device::{accept_device_connections, run_device_client};
use env_logger::Env;
use lazy_static::lazy_static;
use log::{error, info};
use server::spawn_server;

use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::sleep;

type DeviceId = Proquint<u16>;
type SessionId = Proquint<u32>;
type DeviceConnection = Arc<RwLock<tokio::net::TcpStream>>;
type Result<T> = std::result::Result<T, error::Error>;


lazy_static! {
	static ref DEVICE_CONNECTIONS: Arc<RwLock<HashMap<DeviceId, DeviceConnection>>> =
		Arc::new(RwLock::new(HashMap::new()));
	static ref PENDING_CONNECTIONS: Arc<RwLock<HashMap<SessionId, tokio::sync::oneshot::Sender<tokio::net::TcpStream>>>> =
		Arc::new(RwLock::new(HashMap::new()));
	static ref LOCAL_ADDR: String =
		env::var("LOCAL_ADDR").unwrap_or_else(|_| "127.0.0.1:80".into());
}


mod client;
mod device;
mod error;
mod server;


/// WARNING we need to change this later
fn get_device_id() -> DeviceId {
	env::var("DEVICE_ID")
		.ok()
		.and_then(|v| DeviceId::from_quint(&v).ok())
		.unwrap_or_else(|| DeviceId::default())
}


#[tokio::main]
async fn main() -> Result<()> {
	// LASNA_MODE can be:
	// 'server'
	// <domain> or <ip> (something that includes a '.')
	// any other random string like 'none' which won't do anything
	let lasna_mode = env::var("LASNA_MODE").unwrap_or("none".into());


	let env = Env::default()
		.filter_or("LOG_LEVEL", "info")
		.write_style_or("LOG_STYLE", "auto");

	env_logger::init_from_env(env);
	log_env();
	print!(
		"\
	Hi, I'm Lasna ☄️!\n\
    \n\
	I make a tunnel between the remote server and this device.\n\
	So you can access your device from the world wide web.\n\
	\n\
	Our current mode is '{lasna_mode}'.\n\
	\n\
	"
	);

	let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());


	if lasna_mode == "server" {
		// Spawn the device/client connection handlers
		let device_addr = "0.0.0.0:7000"; // Port for devices to connect
		let client_addr = "0.0.0.0:7001"; // Port for clients to connect (Nginx will forward to this port)

		let shutdown_devices = shutdown_rx.clone();
		let shutdown_clients = shutdown_rx.clone();
		tokio::spawn(async move {
			if let Err(e) = accept_device_connections(device_addr, shutdown_devices).await {
				eprintln!("Error in device connection handler: {}", e);
			}
		});

		tokio::spawn(async move {
			if let Err(e) = accept_client_connections(client_addr, shutdown_clients).await {
				eprintln!("Error in client connection handler: {}", e);
			}
		});

		// Also spawn the server that replies with device statuses
		spawn_server(shutdown_rx).await;
	} else if lasna_mode.contains(".") {
		let server_addr = lasna_mode + ":7000";
		let device_id = get_device_id(); // Implement this function to retrieve the device's ID
		info!("Device id: {device_id}");
		tokio::spawn(async move {
			loop {
				if let Err(err) =
					run_device_client(&server_addr, device_id, shutdown_rx.clone()).await
				{
					error!("{err}");
					println!("Error connecting; waiting 10s to retry...");
					sleep(Duration::from_secs(10)).await;
				} else {
					// OK means shutdown was called on client
					break;
				}
			}
		});
	}

	wait_terminate().await;
	shutdown_tx.send(()).unwrap();
	info!("Sent shutdown signal");

	Ok(())
}
