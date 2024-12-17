use common::proquint::Proquint;

use log::{error, info, trace};
use socket2::TcpKeepalive;
use tokio::select;
use tokio::time::sleep;

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;


type DeviceId = Proquint<u16>;
type DeviceConnection = Arc<RwLock<tokio::net::TcpStream>>;

use crate::error::Error;
use crate::{SessionId, DEVICE_CONNECTIONS, LOCAL_ADDR, PENDING_CONNECTIONS};
type Result<T> = std::result::Result<T, Error>;


pub async fn accept_device_connections(
	addr: &str,
	mut shutdown: tokio::sync::watch::Receiver<()>,
) -> Result<()> {
	let listener = TcpListener::bind(addr).await?;
	loop {
		select! {
			r = listener.accept() => {
				let (socket,_) = r?;
				let shutdown = shutdown.clone();
				tokio::spawn(async move {
					if let Err(e) = handle_device_connection(socket, shutdown).await {
						error!("Error handling device connection: {}", e);
					}
				});
			}
			_ = shutdown.changed() => {
				break Ok(());
			}
		}
	}
}

// Handle device connections (initial device connection or new connection for a session)
async fn handle_device_connection(
	mut socket: tokio::net::TcpStream,
	mut shutdown: tokio::sync::watch::Receiver<()>,
) -> Result<()> {
	let mut reader = tokio::io::BufReader::new(&mut socket);
	let mut first_line = String::new();
	reader.read_line(&mut first_line).await?;

	if first_line.starts_with("DEVICE ") {
		// Initial device connection
		let device_id_str = first_line["DEVICE ".len()..].trim();
		let device_id = DeviceId::from_quint(device_id_str)?;

		// Lower socket keepalive timings, just in case of network down
		// or power supply issues.
		{
			let socket = socket2::SockRef::from(&socket);
			// socket.set_tcp_user_timeout(timeout)
			socket.set_tcp_keepalive(
				&TcpKeepalive::new()
					.with_time(Duration::from_secs(60))
					.with_interval(Duration::from_secs(1))
					.with_retries(3),
			)?;
		}

		// Wrap the socket in an Arc<RwLock<>> as DeviceConnection
		let device_conn = Arc::new(RwLock::new(socket));

		// Store the device connection
		{
			let mut devices = DEVICE_CONNECTIONS.write().await;
			devices.insert(device_id.clone(), device_conn.clone());
			info!("Device connected: {}", device_id.to_quint());
		}

		// Monitor socket for when it closes
		loop {
			select! {
				_ = sleep(Duration::from_secs(1)) => {}
				_ = shutdown.changed() => {break;}
			}

			let device_conn = device_conn.write().await;
			let mut buf = [0u8; 32];
			// if device_conn.readable().await.is_err() {
			//   break;
			// }
			match device_conn.try_read(&mut buf) {
				Ok(0) => {
					// Connection closed by the device
					break;
				}
				Ok(_) => {
					// Data received from the device (if any)
					// Can be processed if necessary
				}
				Err(e) => {
					if e.kind() == std::io::ErrorKind::WouldBlock {
						// Only means the read would block
					} else {
						// An error occurred while reading, assume connection closed
						break;
					}
				}
			}
		}

		info!("Device disconnected: {}", device_id.to_quint());
		let mut devices = DEVICE_CONNECTIONS.write().await;
		devices.remove(&device_id);

		Ok(())
	} else if first_line.starts_with("SESSION ") {
		// New connection from device for a pending session
		let session_id = SessionId::from_quint(first_line["SESSION ".len()..].trim())?;

		// Remove the session from PENDING_CONNECTIONS and send the socket
		if let Some(tx) = PENDING_CONNECTIONS.write().await.remove(&session_id) {
			// Send the socket over the oneshot channel
			let _ = tx.send(socket);
			trace!("Session connected: {}", session_id.to_quint());
		} else {
			// No pending session with this session_id
			// Close the socket
			error!(
				"No pending session for session_id {}",
				session_id.to_quint()
			);
			// You might want to send an error message to the device
		}

		Ok(())
	} else {
		// Unknown connection type
		error!("Unknown connection type: {}", first_line.trim());
		// Close the socket
		Ok(())
	}
}

pub async fn run_device_client(
	server_addr: &str,
	device_id: DeviceId,
	mut shutdown: tokio::sync::watch::Receiver<()>,
) -> Result<()> {
	use log::{error, info};
	use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
	use tokio::net::TcpStream;

	// Connect to the server (control channel)
	let mut control_stream = TcpStream::connect(server_addr)
		.await
		.map_err(Error::ServerConnection)?;

	// Send the device ID to register
	let register_message = format!("DEVICE {}\n", device_id.to_quint());
	control_stream
		.write_all(register_message.as_bytes())
		.await?;
	control_stream.flush().await?;
	info!("Registered");

	// Create a BufReader to read lines from the control stream
	let mut control_reader = BufReader::new(control_stream);

	loop {
		// Read messages from the server
		let mut message = String::new();
		select! {
			res = control_reader.read_line(&mut message) => {
				match res {
					Ok(0) => {
						// Server closed the connection
						return Err(Error::ConnectionClosed);
					}
					Ok(_) => {
						let message = message.trim_end();
						if message.starts_with("NEW_CONNECTION ") {
							// Extract the session ID
							let session_id = message["NEW_CONNECTION ".len()..].trim().to_string();

							// Open a new connection to the server for this session
							let mut session_stream = TcpStream::connect(server_addr)
								.await
								.map_err(Error::ServerConnection)?;

							// Send the SESSION message with the session ID
							let session_message = format!("SESSION {}\n", session_id);
							session_stream.write_all(session_message.as_bytes()).await?;
							session_stream.flush().await?;
							trace!("Session {session_id}: Opened");

							// Connect to the local service (e.g., Nginx on port 80)
							let local_addr = LOCAL_ADDR.as_str();
							let mut local_stream = TcpStream::connect(local_addr)
								.await
								.map_err(Error::NginxConnection)?;

							// Forward data between the server and the local service
							// Spawn tasks to handle bidirectional copying
							tokio::spawn(async move {
								match io::copy_bidirectional(&mut local_stream, &mut session_stream).await {
									Ok((from_a, from_b)) => {
										trace!("Session {session_id} success: Sent {from_a} bytes; Received {from_b} bytes.")
									}
									Err(err) => {
										error!("Session {session_id} I/O error: {err}")
									}
								}
							});
						} else {
							return Err(Error::Other(format!(
								"Received unknown message on control channel: {}",
								message
							)));
						}
					}
					Err(e) => {
						return Err(Error::Other(format!(
							"Error reading from control connection: {}",
							e
						)));
					}
				}
			}
			_ = shutdown.changed() => {return Ok(());}
		}
	}
}
