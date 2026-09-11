use chrono::{DateTime, Local, Utc};

pub fn parse_iso(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|d| d.with_timezone(&Utc))
        .ok()
}

pub fn clock(value: &str) -> String {
    match parse_iso(value) {
        Some(t) => {
            let local = t.with_timezone(&Local);
            let today = Local::now().date_naive();
            if local.date_naive() == today {
                local.format("%-I:%M %p").to_string()
            } else {
                local.format("%a %-I:%M %p").to_string()
            }
        }
        None => value.to_string(),
    }
}

pub fn until(value: &str) -> String {
    let Some(t) = parse_iso(value) else { return String::new() };
    let secs = (t - Utc::now()).num_seconds();
    if secs <= 0 {
        return "now".to_string();
    }
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours >= 24 {
        format!("{}d {}h", hours / 24, hours % 24)
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

pub fn age(seconds: f64) -> String {
    let s = seconds.max(0.0) as i64;
    if s < 60 {
        format!("{s}s ago")
    } else if s < 3600 {
        format!("{}m ago", s / 60)
    } else {
        format!("{}h ago", s / 3600)
    }
}
