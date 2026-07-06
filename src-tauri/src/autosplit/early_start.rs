pub fn parse_livesplit_time_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() || s == "-" {
        return None;
    }
    let neg = s.starts_with('-');
    let body = s.strip_prefix('-').unwrap_or(s);
    let mut secs = 0f64;
    let mut unit = 1f64; // seconds, then minutes (×60), then hours (×60)
    for part in body.split(':').rev() {
        let v: f64 = part.parse().ok()?;
        secs += v * unit;
        unit *= 60.0;
    }
    let ms = (secs * 1000.0).round() as i64;
    Some(if neg { -ms } else { ms })
}

pub fn run_start_from_elapsed(now_ms: i64, elapsed_ms: i64) -> i64 {
    now_ms - elapsed_ms
}