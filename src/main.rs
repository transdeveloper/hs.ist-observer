use std::net::TcpListener;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::fs::{File, OpenOptions};
use std::sync::{Arc, Mutex};
use std::thread::{sleep, spawn};

fn main() {
	let mut initial_read = String::new();
	if let Ok(file) = OpenOptions::new().read(true).open("observed.jsonl") {
		let reader = BufReader::new(file);
		for line in reader.lines().flatten() {
			if !line.trim().is_empty() {
				initial_read.push_str(&line);
				initial_read.push('\n');
			}
		}
	}else {
		File::create("observed.jsonl").unwrap();
	}

	let shared_output = Arc::new(Mutex::new(initial_read));
	let timer_output_close = Arc::clone(&shared_output);

	spawn(move || {
		loop {
			let curl_command = match Command::new("curl").arg("https://status.hackerspace.ist").output(){
				Ok(output) => output,
				Err(e) => {
					eprintln!("LOOP: Failed to execute curl: {}", e);
					sleep(Duration::from_secs(5 * 60));
					continue;
				}
			};

			let curl_response = String::from_utf8_lossy(&curl_command.stdout);

			let current_time_now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

			// let json_end_position = curl_response.rfind('}').unwrap();
			// let json_first_half = &curl_response[..json_end_position];

			if let Some(json_end_position) = curl_response.rfind('}') {
				let json_first_half = &curl_response[..json_end_position];
				let updated_text = format!("{},\"timeObserved\":\"{}\"}}", json_first_half, current_time_now);
				let mut output_buffer = timer_output_close.lock().unwrap();

				// output_buffer.clear();
				output_buffer.push_str(&updated_text);
				output_buffer.push('\n');
				if let Ok(file) = OpenOptions::new().append(true).open("observed.jsonl") {
					writeln!(&file, "{}", &updated_text).unwrap();
					println!("LOOP: Appended new event to history");
				}else{
					println!("LOOP: Couldn't write to observed.jsonl");
				}

			}else{
				println!("LOOP: Bad data from hackerspace api?");
			}

			sleep(Duration::from_secs(5*60));
		}
	});

	let bind = "127.0.0.1:8080";
	let listener = match TcpListener::bind(bind) {
		Ok(output) => output,
		Err(e) => {
			eprintln!("Failed to bind to {}: {}", bind, e);
			return;
		}
	};

	println!("Webserver listening on http://127.0.0.1:8080/");

	for stream in listener.incoming() {
		if let Ok(mut stream) = stream {
			let mut buffer = [0; 1024];
			let _ = stream.read(&mut buffer).unwrap();
			let request_string = String::from_utf8_lossy(&buffer);
			// let http_stream = stream.read(&mut buffer).unwrap();
			let mut http_response = "HTTP/1.1 404 NOT FOUND\r\n\
				Content-Length: 9\r\n\
				Connection: close\r\n\
				\r\n\
				Not Found".to_string();

			if request_string.contains("GET / HTTP") {
				let historical_data = shared_output.lock().unwrap().clone();

				if !historical_data.is_empty() {
					http_response = format!("\
					HTTP/1.1 200 OK\r\n\
					Content-Type: application/jsonl\r\n\
					Content-Length: {}\r\n\
					Connection: close\r\n\
					\r\n\
					{}", historical_data.len(), historical_data);
				} else {
					http_response = String::from("\
					HTTP/1.1 204 No Content\r\n\
					Content-Length: 0\r\n\
					Connection: close\r\n\
					\r\n");
				}
			}
			stream.write_all(http_response.as_bytes()).expect("An unexpected error occurred while writing to socket");
		}
	}
}