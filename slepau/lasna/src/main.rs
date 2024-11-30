// main.rs

use client::accept_client_connections;
use common::proquint::{Proquint, QuintError};
use common::utils::log_env;
use device::{accept_device_connections, run_device_client};
use env_logger::Env;
use lazy_static::lazy_static;
use log::{error, info};

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
	static ref DEVICE_CONNECTIONS: RwLock<HashMap<DeviceId, DeviceConnection>> =
		RwLock::new(HashMap::new());
	static ref PENDING_CONNECTIONS: RwLock<HashMap<SessionId, tokio::sync::oneshot::Sender<tokio::net::TcpStream>>> =
		RwLock::new(HashMap::new());
}


mod client;
mod device;
mod error;


/// WARNING we need to change this later
fn get_device_id() -> DeviceId {
	DeviceId::default() // For now we'll do a random value
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


	if lasna_mode == "server" {
		let device_addr = "0.0.0.0:7000"; // Port for devices to connect
		let client_addr = "0.0.0.0:4000"; // Port for clients to connect (Nginx will forward to this port)

		tokio::spawn(async move {
			if let Err(e) = accept_device_connections(device_addr).await {
				eprintln!("Error in device connection handler: {}", e);
			}
		});

		accept_client_connections(client_addr).await?;
	} else if lasna_mode.contains(".") {
		let server_addr = lasna_mode + ":7000";
		let device_id = get_device_id(); // Implement this function to retrieve the device's ID
		info!("Device id: {device_id}");

		loop {
			if let Err(err) = run_device_client(&server_addr, device_id).await {
				error!("{err}");
				println!("Error connecting; waiting 10s to retry...");
				sleep(Duration::from_secs(10)).await;
			}
		}
	}
	Ok(())
}
