pub const POWER_BUCKET_SECONDS: usize = 3;
pub const CADENCE_BUCKET_SECONDS: usize = 5;

pub fn average_into_buckets(values: &[i32], bucket_size: usize) -> Vec<i32> {
    values
        .chunks(bucket_size)
        .map(|chunk| (chunk.iter().sum::<i32>() as f64 / chunk.len() as f64).round() as i32)
        .collect()
}
