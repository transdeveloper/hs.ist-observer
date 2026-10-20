use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use chrono_tz::Europe::Istanbul;
use serde_json::{json, Value};
use std::collections::HashMap;
use serde_json::Value::Null;

pub fn calculate_averages(history: &str) -> String {
	let mut observations: Vec<(DateTime<chrono_tz::Tz>, bool)> =
		history
			.lines()
			.filter_map(|line| {
				let value: Value = serde_json::from_str(line).ok()?;

				let timestamp: i64 =
					value["timeObserved"].as_str()?.parse().ok()?;

				let open = value["open"].as_bool()?;

				let time = Utc
					.timestamp_opt(timestamp, 0)
					.single()?
					.with_timezone(&Istanbul);

				Some((time, open))
			})
			.collect();

	observations.sort_by_key(|(time, _)| *time);

	let mut daily: HashMap<chrono::NaiveDate, i64> = HashMap::new();

	for pair in observations.windows(2) {
		let (start, is_open) = &pair[0];
		let (end, _) = &pair[1];

		let gap = end.signed_duration_since(*start);

		if !is_open || gap <= Duration::zero()
			|| gap > Duration::minutes(10)
		{
			continue;
		}

		let mut cursor = *start;

		while cursor < *end {
			let date = cursor.date_naive();

			let midnight = match date
				.succ_opt()
				.and_then(|d| d.and_hms_opt(0, 0, 0))
				.and_then(|dt| Istanbul.from_local_datetime(&dt).single())
			{
				Some(t) => t,
				None => break,
			};

			let segment_end = if midnight < *end {
				midnight
			} else {
				*end
			};

			let seconds = segment_end
				.signed_duration_since(cursor)
				.num_seconds();

			*daily.entry(date).or_insert(0) += seconds;
			cursor = segment_end;
		}
	}

	let names = [
		"Monday", "Tuesday", "Wednesday", "Thursday",
		"Friday", "Saturday", "Sunday",
	];

	let mut totals = [0_i64; 7];
	let mut counts = [0_u32; 7];

	for (date, seconds) in daily {
		let i = date.weekday().num_days_from_monday() as usize;
		totals[i] += seconds;
		counts[i] += 1;
	}

	let mut result = serde_json::Map::new();

	for i in 0..7 {
		let average = if counts[i] == 0 {
			json!(Null)
		} else {
			json!(
                ((totals[i] as f64 / counts[i] as f64 / 60.0) * 1000.0).round() / 1000.0 // minutes
            )
		};

		result.insert(names[i].to_string(), average);
	}

	Value::Object(result).to_string()
}
