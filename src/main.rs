mod http;

use std::net::TcpListener;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::fs::{File, OpenOptions};
use std::sync::{Arc, Mutex};
use std::thread::{sleep, spawn};
// use crate::http::request_handler::process;

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

	let shared_outp = Arc::new(Mutex::new(initial_read));
	let timer_output_close = Arc::clone(&shared_outp);

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
				{
					let mut output_buffer = timer_output_close.lock().unwrap();
					// output_buffer.clear();
					output_buffer.push_str(&updated_text);
					output_buffer.push('\n');
				}

				if let Ok(file) = OpenOptions::new().append(true).open("observed.jsonl") {
					if let Err(e) = writeln!(&file, "{}", &updated_text){
						eprintln!("LOOP: Failed to write to file: {}", e);
					}
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

	let bind = "0.0.0.0:8080";
	let listener = match TcpListener::bind(bind) {
		Ok(output) => output,
		Err(e) => {
			eprintln!("Failed to bind to {}: {}", bind, e);
			return;
		}
	};

	println!("Webserver listening on http://{}/", bind);
	http::request_handler::serve(listener, shared_outp);
}