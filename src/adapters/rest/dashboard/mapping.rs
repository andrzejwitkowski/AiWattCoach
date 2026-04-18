use crate::domain::training_load::{
    TrainingLoadDashboardPoint, TrainingLoadDashboardRange, TrainingLoadDashboardReport,
    TrainingLoadDashboardSummary, TrainingLoadTsbZone,
};

use super::dto::{
    TrainingLoadDashboardPointDto, TrainingLoadDashboardResponseDto,
    TrainingLoadDashboardSummaryDto,
};

pub(crate) fn map_dashboard_report_to_dto(
    report: TrainingLoadDashboardReport,
) -> TrainingLoadDashboardResponseDto {
    TrainingLoadDashboardResponseDto {
        range: map_range(report.range),
        window_start: report.window_start,
        window_end: report.window_end,
        has_training_load: report.has_training_load,
        summary: map_summary(report.summary),
        points: report.points.into_iter().map(map_point).collect(),
    }
}

fn map_range(range: TrainingLoadDashboardRange) -> &'static str {
    match range {
        TrainingLoadDashboardRange::Last90Days => "90d",
        TrainingLoadDashboardRange::Season => "season",
        TrainingLoadDashboardRange::AllTime => "all-time",
    }
}

fn map_summary(summary: TrainingLoadDashboardSummary) -> TrainingLoadDashboardSummaryDto {
    TrainingLoadDashboardSummaryDto {
        current_ctl: summary.current_ctl,
        current_atl: summary.current_atl,
        current_tsb: summary.current_tsb,
        ftp_watts: summary.ftp_watts,
        average_if_28d: summary.average_if_28d,
        average_ef_28d: summary.average_ef_28d,
        load_delta_ctl_14d: summary.load_delta_ctl_14d,
        tsb_zone: map_tsb_zone(summary.tsb_zone),
    }
}

fn map_point(point: TrainingLoadDashboardPoint) -> TrainingLoadDashboardPointDto {
    TrainingLoadDashboardPointDto {
        date: point.date,
        daily_tss: point.daily_tss,
        ctl: point.ctl,
        atl: point.atl,
        tsb: point.tsb,
    }
}

fn map_tsb_zone(zone: TrainingLoadTsbZone) -> &'static str {
    match zone {
        TrainingLoadTsbZone::FreshnessPeak => "freshness_peak",
        TrainingLoadTsbZone::OptimalTraining => "optimal_training",
        TrainingLoadTsbZone::HighRisk => "high_risk",
    }
}
