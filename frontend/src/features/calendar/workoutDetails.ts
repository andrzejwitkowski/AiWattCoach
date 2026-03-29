import type { IntervalActivity, IntervalEvent } from '../intervals/types';

export type WorkoutDetailSelection = {
  dateKey: string;
  event: IntervalEvent | null;
  activity: IntervalActivity | null;
};

export type WorkoutBar = {
  height: number;
  color: string;
};

const POWER_ZONE_COLORS: Record<number, string> = {
  1: '#6b7280',
  2: '#00e3fd',
  3: '#52c41a',
  4: '#d2ff9a',
  5: '#facc15',
  6: '#ff7351',
  7: '#800020',
};

export function buildPlannedWorkoutBars(event: IntervalEvent): WorkoutBar[] {
  const segments = event.eventDefinition.segments ?? [];
  if (segments.length === 0) {
    return event.eventDefinition.intervals.map((interval, index, all) => ({
      height: 35 + Math.round((index / Math.max(1, all.length - 1)) * 55),
      color: POWER_ZONE_COLORS[interval.zoneId ?? 2] ?? POWER_ZONE_COLORS[2],
    }));
  }

  return segments.map((segment) => ({
    height: heightForPercent(segment.targetPercentFtp),
    color: POWER_ZONE_COLORS[segment.zoneId ?? 2] ?? POWER_ZONE_COLORS[2],
  }));
}

export function buildCompletedWorkoutBars(activity: IntervalActivity): WorkoutBar[] {
  const intervals = activity.details.intervals.filter((interval) => interval.averagePowerWatts !== null);
  if (intervals.length > 0) {
    return intervals.slice(0, 12).map((interval) => ({
      height: heightForPower(interval.averagePowerWatts ?? activity.metrics.averagePowerWatts ?? 0),
      color: POWER_ZONE_COLORS[interval.zone ?? inferZoneFromPower(interval.averagePowerWatts, activity.metrics.ftpWatts)] ?? POWER_ZONE_COLORS[4],
    }));
  }

  const values = extractPowerValues(activity).slice(0, 24);
  if (values.length === 0) {
    return [];
  }

  return values.map((value) => ({
    height: heightForPower(value),
    color: POWER_ZONE_COLORS[inferZoneFromPower(value, activity.metrics.ftpWatts)] ?? POWER_ZONE_COLORS[4],
  }));
}

export function selectWorkoutDetail(
  dateKey: string,
  event: IntervalEvent | null,
  activity: IntervalActivity | null,
): WorkoutDetailSelection {
  const matchedActivity = event?.actualWorkout?.activityId && activity?.id === event.actualWorkout.activityId
    ? activity
    : null;

  return {
    dateKey,
    event,
    activity: event ? matchedActivity : activity,
  };
}

export function formatDurationLabel(totalSeconds: number | null | undefined): string {
  if (!totalSeconds || totalSeconds <= 0) {
    return '0m';
  }

  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.round((totalSeconds % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${String(minutes).padStart(2, '0')}m`;
  }

  return `${minutes}m`;
}

function extractPowerValues(activity: IntervalActivity): number[] {
  const stream = activity.details.streams.find((item) => item.streamType === 'watts');
  if (!stream || !Array.isArray(stream.data)) {
    return [];
  }

  return stream.data.flatMap((value) => (typeof value === 'number' ? [Math.round(value)] : []));
}

function heightForPercent(percent: number | null | undefined): number {
  if (!percent || percent <= 0) {
    return 25;
  }

  return Math.max(20, Math.min(100, Math.round(percent)));
}

function heightForPower(power: number): number {
  if (!Number.isFinite(power) || power <= 0) {
    return 25;
  }

  return Math.max(20, Math.min(100, Math.round(power / 4)));
}

function inferZoneFromPower(power: number | null | undefined, ftpWatts: number | null | undefined): number {
  if (!power || !ftpWatts || ftpWatts <= 0) {
    return 4;
  }

  const percent = (power / ftpWatts) * 100;
  if (percent <= 55) {
    return 1;
  }
  if (percent < 76) {
    return 2;
  }
  if (percent < 91) {
    return 3;
  }
  if (percent < 106) {
    return 4;
  }
  if (percent < 121) {
    return 5;
  }
  if (percent < 151) {
    return 6;
  }
  return 7;
}
