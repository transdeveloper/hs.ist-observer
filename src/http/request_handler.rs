use std::net::TcpListener;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread::{spawn};
use crate::http::bin::averages::calculate_averages;

pub fn serve(listener: TcpListener, shared_outp: Arc<Mutex<String>>) {
	for stream in listener.incoming() {
		match stream {
			Ok(mut stream) => {
				let thread_shared_outp = Arc::clone(&shared_outp);
				spawn(move || {
					let mut buffer = [0; 4096];
					let mut request = Vec::with_capacity(4096);
					// let _ = stream.read(&mut buffer).unwrap();

					loop {
						let bytes_read = match stream.read(&mut buffer) {
							Ok(0) => return,
							Ok(bytes_read) => bytes_read,
							Err(e) => {
								eprintln!("Failed to read request: {}", e);
								return;
							}
						};

						request.extend_from_slice(&buffer[..bytes_read]);

						if request.len() > 16 * 1024 {
							let _ = stream.write_all(b"\
							HTTP/1.1 431 Request Header Fields Too Large\r\n\
							Content-Length: 0\r\n\
							Connection: close\r\n\
							\r\n\\");
							return;
						}

						if request.windows(4).any(|window| window == b"\r\n\r\n") {
							break;
						}
					}

					let request_string = String::from_utf8_lossy(&request);
					// let http_stream = stream.read(&mut buffer).unwrap();

					let (status, body) = if request_string.starts_with("GET /averages HTTP") {
						let history = match thread_shared_outp.lock() {
							Ok(data) => data.clone(),
							Err(_) => return,
						};

						("200 OK", calculate_averages(&history))
					} else if request_string.starts_with("GET / HTTP") {
						let history = match thread_shared_outp.lock() {
							Ok(data) => data.clone(),
							Err(_) => return,
						};

						if history.is_empty() {
							("204 No Content", String::new())
						} else {
							("200 OK", history)
						}
					} else {
						("404 Not Found", "{\"error\":\"Resource not found\"}".to_string())
					};

					let response = format!(
						"HTTP/1.1 {}\r\nContent-Type: application/jsonl\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
						status,
						&body.len(),
						body
					);

					let _ = stream.write_all(response.as_bytes());
				});
			}
			Err(e) => {
				eprintln!("Failed to accept connection: {}", e);
			}
		}
	}
}