use chrono::prelude::*;
use chrono::{DateTime, Duration, NaiveDateTime};
use clap::Parser;
use ical::parser::ical::component::IcalCalendar;
use std::collections::HashSet;
use std::fs::read_to_string;
use std::fs::File;
use std::io::BufReader;
use std::{thread, time};
use wsgg::{ChatMessage, Connection};

/// Calendar app
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Location of the file containing the cookie for the bot to use
    #[arg(long)]
    cookie: String,

    /// Use the dev environement (chat2.strims.gg)
    #[arg(long, default_value_t = false)]
    dev: bool,

    /// Specifies the directory to look for .ical calendars for
    #[arg(long, default_value_t = String::from("cals"))]
    cal_dir: String,
}

struct App {
    cals: Vec<IcalCalendar>,
}

impl App {
    fn new() -> App {
        App { cals: Vec::new() }
    }

    fn add_cal(&mut self, path: String) {
        let buf = BufReader::new(File::open(&path).unwrap());
        let reader = ical::IcalParser::new(buf);
        for cal in reader {
            match cal {
                Ok(c) => self.cals.push(c),
                Err(e) => eprintln!(
                    "Could not parse calendar: [file=\"{}\", parse error=\"{}\"]",
                    path, e
                ),
            }
        }
    }

    fn search(&self, search_term: String, time: DateTime<Utc>) -> Option<(String, DateTime<Utc>)> {
        let search_term = search_term.to_lowercase();
        let mut found: Vec<_> = Vec::new();
        for cal in &self.cals {
            for event in &cal.events {
                for property in &event.properties {
                    if property.name != "SUMMARY" {
                        continue;
                    }

                    let property = match property.value.as_ref() {
                        Some(p) => p,
                        None => continue,
                    };

                    if property.to_lowercase().contains(&search_term) {
                        found.push(&event.properties);
                    }
                }
            }
        }

        let mut first = Utc.with_ymd_and_hms(3023, 3, 4, 13, 0, 0).unwrap();
        let mut res: Option<String> = None;

        for event in found {
            let start = event
                .iter()
                .filter(|x| x.name == "DTSTART")
                .collect::<Vec<_>>()[0]
                .value
                .as_ref();
            let summary = event
                .iter()
                .filter(|x| x.name == "SUMMARY")
                .collect::<Vec<_>>()[0]
                .value
                .as_ref();

            if start == None {
                continue;
            }
            // Wil only support this until 3022
            let start = start.unwrap();
            let dt = NaiveDateTime::parse_from_str(start, "%Y%m%dT%H%M%SZ").unwrap_or_default();
            let dt = Utc.from_local_datetime(&dt).unwrap();
            if dt < time {
                continue;
            }

            if dt < first {
                first = dt;
                res = Some(summary.unwrap().clone());
            }
        }
        if res == None {
            return None;
        }
        Some((res.unwrap(), first))
    }

    fn format_duration(duration: Duration) -> String {
        let d = duration.num_days();
        let h = duration.num_hours() - d * 24;
        let m = (duration.num_minutes() - h * 60) % 60;
        if d == 0 && h == 0 {
            return format!("{m} Minutes");
        }
        if m == 0 && h == 0 {
            return format!("{d} Days");
        }
        if m == 0 && d == 0 {
            return format!("{h} Hours");
        }
        if h == 0 {
            return format!("{d} Days and {m} Minutes");
        }
        if m == 0 {
            return format!("{d} Days and {h} Hours");
        }
        if d == 0 {
            return format!("{h} Hours and {m} Minutes");
        }

        return format!("{d} Days {h} Hours and {m} Minutes");
    }
}

fn get_icals(dir: String) -> Result<HashSet<String>, String> {
    let mut res = HashSet::new();
    for entry in std::fs::read_dir(&dir).expect("Could not iterate calendar directory") {
        let entry = entry.expect("Something horrible happened with this entry: {entry}");
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let extension = match path.extension() {
            Some(e) => e,
            _ => continue,
        };

        if extension != "ics" {
            continue;
        }
        let filename = entry
            .file_name()
            .into_string()
            .expect("Somethings weird about {entry.file_name()}");

        res.insert(format!("{dir}/{filename}"));
    }
    Ok(res)
}

fn main() {
    let bot_account = "whenis";

    let args = Args::parse();
    let mut app = App::new();

    let icals = get_icals(args.cal_dir).expect("Could not read cal dir");
    for ical in icals {
        app.add_cal(ical);
    }

    let cookie: String = read_to_string(args.cookie).unwrap().parse().unwrap();

    let mut conn = if args.dev {
        println!("Running in test environement");
        Connection::new_dev(cookie.as_str()).unwrap()
    } else {
        println!("Running in production environement");
        Connection::new(cookie.as_str()).unwrap()
    };

    loop {
        let msg: ChatMessage = match conn.read_msg() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Error: {e}");
                continue;
            }
        };

        let data: String = msg.message.to_string();

        if data.starts_with(bot_account) {
            let rest = data
                .strip_prefix(format!("{bot_account} ").as_str())
                .unwrap()
                .to_string();

            let now = Utc::now();

            if let Some((event, dt)) = app.search(rest, now) {
                let duration = dt.signed_duration_since(now);
                let formatted_duration = App::format_duration(duration);

                let message = format!("{formatted_duration} until {event}");

                // 300ms is the throtteling threshold
                // Could be 0 if used with a bot
                let slep = time::Duration::from_millis(500);
                thread::sleep(slep);

                match conn.send(&message) {
                    Err(e) => eprintln!("Error while sending: {e}"),
                    Ok(_) => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::App;
    use chrono::Duration;
    use chrono::TimeZone;
    use chrono::Utc;

    #[test]
    fn search_first_quali() {
        let mut app = App::new();
        app.add_cal("cals/f1_23.ics".to_string());
        let dt = Utc.with_ymd_and_hms(2023, 3, 4, 13, 0, 0).unwrap(); // `2023-03-04T13:00:00Z`
        let expected = "⏱️ FORMULA 1 GULF AIR BAHRAIN GRAND PRIX 2023 - Qualifying".to_string();
        let (res, _) = app.search("quali".to_string(), dt).unwrap();
        assert_eq!(res, expected.clone());
        let (res, _) = app.search("qualifying".to_string(), dt).unwrap();
        assert_eq!(res, expected);
    }
    #[test]
    fn search_second_quali() {
        let mut app = App::new();
        app.add_cal("cals/f1_23.ics".to_string());
        let dt = Utc.with_ymd_and_hms(2023, 3, 4, 17, 0, 0).unwrap();
        let expected =
            "⏱\u{fe0f} FORMULA 1 STC SAUDI ARABIAN GRAND PRIX 2023 - Qualifying".to_string();
        let (res, _) = app.search("quali".to_string(), dt).unwrap();
        assert_eq!(res, expected.clone());
        let (res, _) = app.search("qualifying".to_string(), dt).unwrap();
        assert_eq!(res, expected);
    }

    #[test]
    fn duration_days() {
        let d = Duration::days(7);
        let h = Duration::hours(7);
        let m = Duration::minutes(7);
        let duration = d + h + m;
        assert_eq!(
            App::format_duration(duration),
            "7 Days 7 Hours and 7 Minutes"
        );
        let duration = d + m;
        assert_eq!(App::format_duration(duration), "7 Days and 7 Minutes");
    }
    #[test]
    fn duration_hours() {
        let h = Duration::hours(7);
        let m = Duration::minutes(7);
        let duration = h + m;
        assert_eq!(App::format_duration(duration), "7 Hours and 7 Minutes");
    }
    #[test]
    fn duration_minutes() {
        let m = Duration::minutes(7);
        let duration = m;
        assert_eq!(App::format_duration(duration), "7 Minutes");
    }
}
