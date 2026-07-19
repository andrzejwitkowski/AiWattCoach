use std::sync::Once;

use plotters::prelude::*;

use crate::domain::completed_workouts::{CompletedWorkout, CompletedWorkoutSeries};

const ROLLING_WINDOW: usize = 3;
const CHART_WIDTH: u32 = 1200;
const CHART_HEIGHT: u32 = 500;

// ponytail: ab_glyph backend has no system font lookup; embed one TTF so builds are reproducible.
const FONT_BYTES: &[u8] = include_bytes!("DejaVuSans.ttf");
static REGISTER_FONT: Once = Once::new();

fn ensure_font_registered() {
    REGISTER_FONT.call_once(|| {
        // ponytail: font bytes are embedded and valid; ignore the opaque InvalidFont error.
        let _ = plotters::style::register_font("sans-serif", FontStyle::Normal, FONT_BYTES);
    });
}

pub struct PowerChartData {
    pub smoothed: Vec<i32>,
    pub max_watts: i32,
    pub np_watts: i32,
    pub avg_watts: i32,
}

pub fn extract_power_chart_data(workout: &CompletedWorkout) -> Option<PowerChartData> {
    let watts = watts_stream(workout)?;
    if watts.len() < 2 {
        return None;
    }
    let max_watts = *watts.iter().max()?;
    let np_watts = workout.metrics.normalized_power_watts?;
    let avg_watts = workout.metrics.average_power_watts?;
    Some(PowerChartData {
        smoothed: rolling_mean_3s(&watts),
        max_watts,
        np_watts,
        avg_watts,
    })
}

pub fn render_power_chart_png(data: &PowerChartData) -> Vec<u8> {
    ensure_font_registered();
    let mut buf = vec![0u8; (CHART_WIDTH * CHART_HEIGHT * 3) as usize];
    // ponytail: unwrap on in-memory bitmap backend is safe — no I/O can fail.
    let root =
        BitMapBackend::with_buffer(&mut buf, (CHART_WIDTH, CHART_HEIGHT)).into_drawing_area();
    root.fill(&WHITE).unwrap();

    let n = data.smoothed.len() as f64;
    let y_max = (data.max_watts.max(data.np_watts).max(data.avg_watts) + 50) as f64;

    let mut chart = ChartBuilder::on(&root)
        .margin(20)
        .x_label_area_size(30)
        .y_label_area_size(50)
        .build_cartesian_2d(0f64..n, 0f64..y_max)
        .unwrap();

    chart
        .configure_mesh()
        .x_desc("Time (s)")
        .y_desc("Watts")
        .x_label_formatter(&|v| format_mmss(*v as i64))
        .draw()
        .unwrap();

    let trace: Vec<(f64, f64)> = data
        .smoothed
        .iter()
        .enumerate()
        .map(|(i, &w)| (i as f64, w as f64))
        .collect();
    chart
        .draw_series(LineSeries::new(trace, &RGBColor(30, 30, 30)))
        .unwrap();

    for &(watts, color, label) in &[
        (data.max_watts, &RED, "MAX"),
        (data.np_watts, &BLUE, "NP"),
        (data.avg_watts, &GREEN, "AVG"),
    ] {
        let w = watts as f64;
        chart
            .draw_series(LineSeries::new(vec![(0.0, w), (n, w)], color))
            .unwrap()
            .label(format!("{label} {watts}W"))
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], *color));
    }
    chart.configure_series_labels().draw().unwrap();

    drop(chart);
    root.present().unwrap();
    drop(root);

    let image = image::RgbImage::from_raw(CHART_WIDTH, CHART_HEIGHT, buf)
        .expect("raw buffer size matches chart dimensions");
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("in-memory PNG encoding cannot fail");
    png
}

fn watts_stream(workout: &CompletedWorkout) -> Option<Vec<i32>> {
    let stream = workout
        .details
        .streams
        .iter()
        .find(|s| s.stream_type.eq_ignore_ascii_case("watts"))?;
    let series = stream
        .primary_series
        .as_ref()
        .or(stream.secondary_series.as_ref())?;
    match series {
        CompletedWorkoutSeries::Integers(v) => Some(v.iter().map(|&x| x.max(0) as i32).collect()),
        CompletedWorkoutSeries::Floats(v) => Some(
            v.iter()
                .map(|&x| {
                    if x.is_finite() && x >= 0.0 {
                        x.round() as i32
                    } else {
                        0
                    }
                })
                .collect(),
        ),
        _ => None,
    }
}

fn rolling_mean_3s(samples: &[i32]) -> Vec<i32> {
    let n = samples.len();
    let mut prefix = vec![0i64; n + 1];
    for (i, &v) in samples.iter().enumerate() {
        prefix[i + 1] = prefix[i] + i64::from(v);
    }
    (0..n)
        .map(|i| {
            let start = i.saturating_sub(ROLLING_WINDOW - 1);
            let count = (i - start + 1) as i64;
            ((prefix[i + 1] - prefix[start]) as f64 / count as f64).round() as i32
        })
        .collect()
}

fn format_mmss(seconds: i64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_mean_3s_smooths_correctly() {
        let out = rolling_mean_3s(&[100, 200, 300, 400]);
        assert_eq!(out[0], 100);
        assert_eq!(out[1], 150);
        assert_eq!(out[2], 200);
        assert_eq!(out[3], 300);
    }

    #[test]
    fn extract_returns_none_for_empty_stream() {
        let workout = CompletedWorkout {
            completed_workout_id: "w1".into(),
            user_id: "u1".into(),
            start_date_local: "2026-01-01".into(),
            source_activity_id: None,
            planned_workout_id: None,
            name: None,
            description: None,
            activity_type: None,
            external_id: None,
            trainer: false,
            duration_seconds: None,
            distance_meters: None,
            metrics: Default::default(),
            details: crate::domain::completed_workouts::CompletedWorkoutDetails {
                intervals: vec![],
                interval_groups: vec![],
                streams: vec![],
                interval_summary: vec![],
                skyline_chart: vec![],
                power_zone_times: vec![],
                heart_rate_zone_times: vec![],
                pace_zone_times: vec![],
                gap_zone_times: vec![],
            },
            details_unavailable_reason: None,
            power_curve_5s: None,
        };
        assert!(extract_power_chart_data(&workout).is_none());
    }
}
