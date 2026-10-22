mod http;

use std::net::{SocketAddr, TcpListener};
use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::fs::{File, OpenOptions};
use std::sync::{Arc, Mutex};
use std::thread::{sleep, spawn};
use clap::{Parser};

#[derive(Parser, Debug)]
#[command(version, about = "Server to determine when the local hackerspace is open.")]
struct Args {
	#[arg(short, long, default_value = "127.0.0.1:8080")]
	bind: SocketAddr,
	#[arg(short, long, default_value = "somerandomstring")]
	api_key: String,
}

fn main() {
	let args = Args::parse();

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
	let timer_outp_close = Arc::clone(&shared_outp);

	spawn(move || {
		loop {
			let api_request = match reqwest::blocking::get("https://status.hackerspace.ist"){
				Ok(text) => text,
				Err(e) => {
					eprintln!("Network error while requesting API: {}", e);
					return;
				}
			};

			let api_response = match api_request.text() {
				Ok(text) => text.to_string(),
				Err(e) => {
					eprintln!("Failed to read API Response: {}", e);
					return;
				}
			};

			let current_time_now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

			if let Some(json_end_position) = api_response.rfind('}') {
				let json_first_half = &api_response[..json_end_position];
				let updated_text = format!("{},\"timeObserved\":\"{}\"}}", json_first_half, current_time_now);

				// comment: this should make it so we release the lock by running the push operation in a different scope.
				// tldr: lock doesn't wait for filesystem operations(make an issue if i'm wrong)
				{
					let mut output_buffer = timer_outp_close.lock().unwrap();
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

	let bind = args.bind;
	let listener = match TcpListener::bind(args.bind) {
		Ok(output) => output,
		Err(e) => {
			eprintln!("Failed to bind to {}: {}", bind, e);
			return;
		}
	};

	println!("Webserver listening on http://{}/", bind);
	http::request_handler::serve(listener, shared_outp, Arc::new(Mutex::new(args)));
}