pub const POWER_BUCKET_SECONDS: usize = 3;
pub const CADENCE_BUCKET_SECONDS: usize = 5;

pub fn average_into_buckets(values: &[i32], bucket_size: usize) -> Vec<i32> {
    if bucket_size == 0 {
        return Vec::new();
    }

    values
        .chunks(bucket_size)
        .map(|chunk| {
            (chunk.iter().map(|&value| i64::from(value)).sum::<i64>() as f64 / chunk.len() as f64)
                .round() as i32
        })
        .collect()
}
