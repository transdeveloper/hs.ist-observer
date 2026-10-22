use std::net::TcpListener;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread::{spawn};
use serde_json::{json};
use crate::Args;
use crate::http::bin::averages::calculate_averages;

pub fn serve(listener: TcpListener, shared_outp: Arc<Mutex<String>>, shared_arguments: Arc<Mutex<Args>>) {
	for stream in listener.incoming() {
		match stream {
			Ok(mut stream) => {
				let thread_shared_outp = Arc::clone(&shared_outp);
				let arguments = Arc::clone(&shared_arguments);

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

					let args = match arguments.lock() {
						Ok(args) => args,
						Err(e) => {
							eprintln!("Error occurred while reading arguments: {}", e);
							return;
						}
					};

					let is_request_authenticated = {
						let authorization = request_string.lines().find_map(|line|{
							let (name, val) = line.split_once(':')?;

							if name.trim().eq_ignore_ascii_case("Authorization") {
								Some(val.trim())
							} else{
								None
							}
						});

						match authorization {
							None => Some(json!({"error": "Authorization header not supplied"})),
							Some(auth_header_val) => {
								let expected = format!("Bearer {}", args.api_key);
								if auth_header_val != expected {
									Some(json!({"error": "Invalid API key supplied"}))
								} else{
									None
								}
							}
						}
					};

					if let Some(response) = is_request_authenticated {
						let body = response.to_string();
						let _ = stream.write_all(format!(
							"HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
							body.len(), body
						).as_bytes());
						return;
					}
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

					let json_body: Vec<serde_json::Value> = match body
						.lines()
						.filter(|line| !line.trim().is_empty())
						.map(|line| serde_json::from_str(line)) // essentially: fn x(line: &str) -> serde_json::Result<serde_json::Value>{ return serde_json::from_str( line ); }
						.collect::<Result<_, _>>() { // Ok, Err?
						Ok(body) => body,
						Err(e) => {
							eprintln!("Failed to parse body: {}", e);
							return;
						}
					};

					let json_string = match serde_json::to_string(&json_body) {
						Ok(json_string) => json_string,
						Err(e) => {
							eprintln!("Failed to serialize body: {}", e);
							return;
						}
					};

					let response = format!(
						"HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
						status,
						&json_string.len(),
						json_string
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