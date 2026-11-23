/// Byte counts in powers of 1024, using B, KB, MB, GB and TB.
pub fn bytes(n: u64) -> String {
    scaled_bytes(n, 1024.0)
}

pub fn decimal_bytes(n: u64) -> String {
    scaled_bytes(n, 1000.0)
}

fn scaled_bytes(n: u64, base: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = n as f64;
    let mut unit = 0;
    while size >= base && unit < UNITS.len() - 1 {
        size /= base;
        unit += 1;
    }
    if unit == 0 {
        return format!("{n} B");
    }
    let amount = format!("{size:.1}");
    format!("{} {}", amount.trim_end_matches(".0"), UNITS[unit])
}
