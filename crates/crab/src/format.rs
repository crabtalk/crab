//! How `crab` writes numbers for a person to read.

use crabtalk_sdk::system::meta;

/// `4,865`, then `3.8M`, `1.2B` from a million up.
pub fn count(n: u64) -> String {
    match n {
        0..1_000_000 => {
            let digits = n.to_string();
            let mut out = String::new();
            for (i, c) in digits.chars().enumerate() {
                if i > 0 && (digits.len() - i).is_multiple_of(3) {
                    out.push(',');
                }
                out.push(c);
            }
            out
        }
        1_000_000..1_000_000_000 => format!("{:.1}M", n as f64 / 1e6),
        1_000_000_000..1_000_000_000_000 => format!("{:.1}B", n as f64 / 1e9),
        _ => format!("{:.1}T", n as f64 / 1e12),
    }
}

/// Bytes in binary units: `1.5 GB`.
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = n as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    match unit {
        0 => format!("{n} B"),
        _ => format!("{size:.1} {}", UNITS[unit]),
    }
}

/// MiB-milliseconds of memory held, as GB-hours: `12.5 GB-h`.
pub fn gb_hours(held: u64) -> String {
    format!("{:.1} GB-h", held as f64 / meta::GB_HOUR as f64)
}

/// USD micros: to the cent from a cent up, `$2.41`; below a cent, to two
/// significant digits, `$0.0013`.
pub fn usd(micros: f64) -> String {
    let dollars = micros / 1e6;
    if micros == 0.0 || micros.abs() >= 10_000.0 {
        return format!("${dollars:.2}");
    }
    let places = (1 - dollars.abs().log10().floor() as i32).max(2) as usize;
    format!("${dollars:.places$}")
}

/// A share of a whole in whole percents, `<1%` between none and one.
pub fn percent(share: f64) -> String {
    match share {
        s if s > 0.0 && s < 0.01 => "<1%".to_owned(),
        s => format!("{}%", (s * 100.0).round()),
    }
}

/// Days since the Unix epoch, or epoch seconds, as `2026-10-18`.
pub fn day(day: i64) -> String {
    date(day * 86_400)
}

pub fn date(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0)
        .map_or_else(|| secs.to_string(), |at| at.format("%Y-%m-%d").to_string())
}

/// Epoch milliseconds as `2026-10-18 14:00 UTC`.
pub fn instant(ms: u64) -> String {
    chrono::DateTime::from_timestamp_millis(ms as i64).map_or_else(
        || ms.to_string(),
        |at| at.format("%Y-%m-%d %H:%M UTC").to_string(),
    )
}
