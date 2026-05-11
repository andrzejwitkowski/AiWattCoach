use serde_json::json;

const MAX_BALANCE_SERIES_POINTS: usize = 256;

pub(super) struct WPrimeBalanceOutput {
    pub(super) date: String,
    pub(super) workout_id: String,
    pub(super) workout_name: Option<String>,
    pub(super) cp_watts: i32,
    pub(super) w_prime_joules: i32,
    pub(super) cp_source: String,
    pub(super) w_prime_source: String,
    pub(super) sample_count: usize,
    pub(super) valid_power_samples: usize,
    pub(super) balance_series: Vec<f64>,
    pub(super) start_balance: f64,
    pub(super) end_balance: f64,
    pub(super) min_balance: f64,
    pub(super) max_deficit: f64,
    pub(super) time_above_90: u32,
    pub(super) time_50_to_90: u32,
    pub(super) time_10_to_50: u32,
    pub(super) time_below_10: u32,
    pub(super) depleted: bool,
}

pub(super) fn build_w_prime_balance_response(output: &WPrimeBalanceOutput) -> String {
    let sampled = sample_balance_series(&output.balance_series, MAX_BALANCE_SERIES_POINTS);

    let mut resp = json!({
        "date": output.date,
        "workout_id": output.workout_id,
        "cp_watts": output.cp_watts,
        "w_prime_joules": output.w_prime_joules,
        "cp_source": output.cp_source,
        "w_prime_source": output.w_prime_source,
        "sample_count": output.sample_count,
        "sample_period_seconds": 1,
        "valid_power_samples": output.valid_power_samples,
        "summary": {
            "start_w_prime_balance": round_1(output.start_balance),
            "end_w_prime_balance": round_1(output.end_balance),
            "min_w_prime_balance": round_1(output.min_balance),
            "max_deficit_joules": round_1(output.max_deficit),
            "time_above_90_percent_seconds": output.time_above_90,
            "time_50_to_90_percent_seconds": output.time_50_to_90,
            "time_10_to_50_percent_seconds": output.time_10_to_50,
            "time_below_10_percent_seconds": output.time_below_10,
            "w_prime_depleted": output.depleted,
        },
        "w_prime_balance_series": sampled,
    });

    if let Some(ref name) = output.workout_name {
        resp["workout_name"] = json!(name);
    }

    resp.to_string()
}

fn sample_balance_series(series: &[f64], max_points: usize) -> Vec<f64> {
    if max_points == 0 || series.is_empty() {
        return Vec::new();
    }
    if max_points == 1 {
        return vec![round_1(*series.last().unwrap_or(&0.0))];
    }
    if series.len() <= max_points {
        return series.iter().map(|&v| round_1(v)).collect();
    }
    let last = series.len() - 1;
    (0..max_points)
        .map(|i| round_1(series[i * last / (max_points - 1)]))
        .collect()
}

fn round_1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}
