use std::fmt;

use common::proquint::QuintError;

// Custom error type
#[derive(Debug)]
pub enum Error {
	Io(std::io::Error),
	NginxConnection(std::io::Error),
	ServerConnection(std::io::Error),
	// Hyper(hyper::Error),
	MissingHostHeader,
	InvalidHostFormat,
	Join(tokio::task::JoinError),
	DeviceStreamRequestFailed,
	ConnectionClosed,
	Proquint(QuintError),
	Utf8(std::str::Utf8Error),
	Other(String),
	// HeaderToStr(hyper::header::ToStrError),
	// Add more variants as needed
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Error::Io(err) => write!(f, "IO error: {}", err),
			Error::NginxConnection(err) => write!(f, "Nginx Connection error: {}", err),
			Error::ServerConnection(err) => write!(f, "Server Connection error: {}", err),
			// Error::Hyper(err) => write!(f, "Hyper error: {}", err),
			Error::MissingHostHeader => write!(f, "Missing Host header"),
			Error::InvalidHostFormat => write!(f, "Invalid host format"),
			Error::ConnectionClosed => write!(f, "Connection closed"),
      Error::DeviceStreamRequestFailed => write!(f, "Device stream request failed"),
			// Error::InvalidResponse => write!(f, "Invalid response"),
			Error::Join(err) => write!(f, "Join error: {}", err),
			// Error::DeviceNotConnected => write!(f, "Device not connected"),
			Error::Proquint(err) => write!(f, "Proquint error: {}", err),
			Error::Utf8(err) => write!(f, "UTF-8 error: {}", err),

			Error::Other(v) => write!(f, "{v}"),
			// Error::HeaderToStr(err) => write!(f, "Header to string error: {}", err),
		}
	}
}

impl std::error::Error for Error {}

// Implement `From` traits for automatic error conversion
impl From<std::io::Error> for Error {
	fn from(err: std::io::Error) -> Error {
		Error::Io(err)
	}
}

// impl From<hyper::Error> for Error {
// 	fn from(err: hyper::Error) -> Error {
// 		Error::Hyper(err)
// 	}
// }

impl From<std::str::Utf8Error> for Error {
	fn from(err: std::str::Utf8Error) -> Error {
		Error::Utf8(err)
	}
}

// impl From<hyper::header::ToStrError> for Error {
// 	fn from(err: hyper::header::ToStrError) -> Error {
// 		Error::HeaderToStr(err)
// 	}
// }
impl From<QuintError> for Error {
	fn from(err: QuintError) -> Self {
		Error::Proquint(err)
	}
}
// Implement From for JoinError
impl From<tokio::task::JoinError> for Error {
	fn from(err: tokio::task::JoinError) -> Error {
		Error::Join(err)
	}
}