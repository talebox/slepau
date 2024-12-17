/// 
/// Handles client connections from server <-> device.
///

use common::proquint::Proquint;

use log::{error, trace};
use tokio::select;
use tokio::time::timeout;

use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

type DeviceId = Proquint<u16>;

use crate::error::Error;
use crate::{SessionId, DEVICE_CONNECTIONS, PENDING_CONNECTIONS};
type Result<T> = std::result::Result<T, Error>;


// Accept client connections as raw TCP streams
pub async fn accept_client_connections(
	addr: &str,
	mut shutdown: tokio::sync::watch::Receiver<()>,
) -> Result<()> {
	let listener = TcpListener::bind(addr).await?;
	loop {
		select! {
			r = listener.accept() => {
				let (socket,_) = r?;
				// let shutdown = shutdown.clone();
				tokio::spawn(async move {
					if let Err(e) = handle_client_connection(socket).await {
						error!("Error handling client connection: {}", e);
					}
				});
			}
			_ = shutdown.changed() => {
				break Ok(());
			}
		}
	}
}
// Handle client connections and forward data
async fn handle_client_connection(mut client_conn: tokio::net::TcpStream) -> Result<()> {
	// Read initial data from the client to extract the Host header
	let mut buffer = [0u8; 8192];
	let n = client_conn.peek(&mut buffer).await?;
	if n == 0 {
		// Connection closed
		return Ok(());
	}

	// Convert the initial data to a string for parsing
	let request_text = String::from_utf8_lossy(&buffer[..n]);

	// Extract the Host header to determine the device ID
	let host_header = extract_host_header(&request_text)?;

	// Decode the device ID from the subdomain
	let device_id = decode_device_id_from_subdomain(&host_header)?;

	// Retrieve the device connection
	let device_connection = {
		let devices = DEVICE_CONNECTIONS.read().await;
		devices.get(&device_id).cloned()
	};

	if let Some(device_conn) = device_connection {
		// Generate a unique session ID
		let session_id = SessionId::default();

		// Create a oneshot channel to receive the new device connection
		let (tx, rx) = oneshot::channel();

		// Insert the sender into PENDING_CONNECTIONS with the session ID
		{
			let mut pending = PENDING_CONNECTIONS.write().await;
			pending.insert(session_id, tx);
		}

		// Send a request over device_conn to the device, including the session ID
		{
			let mut device_conn = device_conn.write().await;
			// Send a message to the device to open a new connection
			// Protocol: "NEW_CONNECTION <session_id>\n"
			let message = format!("NEW_CONNECTION {}\n", session_id);
			device_conn.write_all(message.as_bytes()).await?;
			device_conn.flush().await?;
		}

		// Wait for the device to connect back with the new TCP stream
		let mut device_stream = timeout(Duration::from_secs(30), rx)
			.await
			.map_err(|_| Error::DeviceStreamRequestFailed)?
			.map_err(|_| Error::DeviceStreamRequestFailed)?;

		// Forward data between client and device using the new connection
		match tokio::io::copy_bidirectional(&mut client_conn, &mut device_stream).await {
			Ok((from_a, from_b)) => {
				trace!(
					"Session {session_id} success: Sent {from_a} bytes; Received {from_b} bytes."
				)
			}
			Err(err) => {
				error!("Session {session_id} I/O error: {err}")
			}
		}
	} else {
		// Device not connected; send HTTP 503 response
		let response = "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 20\r\n\r\nDevice not connected";
		client_conn.write_all(response.as_bytes()).await?;
	}

	Ok(())
}

// Extract the Host header from the HTTP request
fn extract_host_header(request_text: &str) -> Result<String> {
	for line in request_text.lines() {
		if line.to_lowercase().starts_with("host:") {
			return Ok(line[5..].trim().to_string());
		}
		if line == "\r" || line.is_empty() {
			// End of headers
			break;
		}
	}
	Err(Error::MissingHostHeader)
}
fn decode_device_id_from_subdomain(host: &str) -> Result<DeviceId> {
	let parts: Vec<&str> = host.split('.').collect();
	if !parts.is_empty() {
		// Pull proquint from part 0 or 1 after host split, allowing for both of these to work:
		// 'losab.talebox.dev' and `chunk.losab.talebox.dev`
		let device_id =
			DeviceId::from_quint(parts[0]).or_else(|_| DeviceId::from_quint(parts[1]))?;
		Ok(device_id)
	} else {
		Err(Error::InvalidHostFormat)
	}
}
