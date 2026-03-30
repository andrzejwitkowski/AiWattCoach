import type { IntervalActivity, IntervalEvent } from '../intervals/types';

export type WorkoutDetailSelection = {
  dateKey: string;
  event: IntervalEvent | null;
  activity: IntervalActivity | null;
};

export type WorkoutBar = {
  height: number;
  color: string;
  widthUnits: number;
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
      widthUnits: normalizeWidthUnits(interval.durationSeconds),
    }));
  }

  return segments.map((segment) => ({
    height: heightForPercent(segment.targetPercentFtp),
    color: POWER_ZONE_COLORS[segment.zoneId ?? 2] ?? POWER_ZONE_COLORS[2],
    widthUnits: normalizeWidthUnits(segment.durationSeconds),
  }));
}

export function buildCompletedWorkoutBars(activity: IntervalActivity): WorkoutBar[] {
  const intervals = activity.details.intervals.filter((interval) => interval.averagePowerWatts !== null);
  if (intervals.length > 0) {
    return intervals.slice(0, 12).map((interval) => ({
      height: heightForPower(interval.averagePowerWatts ?? activity.metrics.averagePowerWatts ?? 0),
      color: POWER_ZONE_COLORS[interval.zone ?? inferZoneFromPower(interval.averagePowerWatts, activity.metrics.ftpWatts)] ?? POWER_ZONE_COLORS[4],
      widthUnits: completedIntervalDurationSeconds(interval),
    }));
  }

  const values = extractCompletedPowerValues(activity).slice(0, 24);
  if (values.length === 0) {
    return [];
  }

  return values.map((value) => ({
    height: heightForPower(value),
    color: POWER_ZONE_COLORS[inferZoneFromPower(value, activity.metrics.ftpWatts)] ?? POWER_ZONE_COLORS[4],
    widthUnits: 1,
  }));
}

export function buildMatchedWorkoutBars(actualWorkout: IntervalEvent['actualWorkout']): WorkoutBar[] {
  if (!actualWorkout?.matchedIntervals.length) {
    return actualWorkout?.powerValues.slice(0, 24).map((value) => ({
      height: heightForPower(value),
      color: POWER_ZONE_COLORS[4],
      widthUnits: 1,
    })) ?? [];
  }

  return actualWorkout.matchedIntervals.map((interval) => ({
    height: heightForPower(interval.averagePowerWatts ?? interval.normalizedPowerWatts ?? 0),
    color: POWER_ZONE_COLORS[interval.zoneId ?? 4] ?? POWER_ZONE_COLORS[4],
    widthUnits: normalizeWidthUnits(matchedIntervalDurationSeconds(interval)),
  }));
}

export function selectWorkoutDetail(
  dateKey: string,
  event: IntervalEvent | null,
  activity: IntervalActivity | IntervalActivity[] | null,
): WorkoutDetailSelection {
  const activities = Array.isArray(activity)
    ? activity
    : activity
      ? [activity]
      : [];
  const matchedActivity = event?.actualWorkout?.activityId
    ? activities.find((candidate) => candidate.id === event.actualWorkout?.activityId) ?? null
    : null;

  return {
    dateKey,
    event,
    activity: event ? matchedActivity : activities[0] ?? null,
  };
}

export function formatDurationLabel(totalSeconds: number | null | undefined): string {
  if (!totalSeconds || totalSeconds <= 0) {
    return '0m';
  }

  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${String(minutes).padStart(2, '0')}m`;
  }

  return `${minutes}m`;
}

export function extractCompletedPowerValues(activity: IntervalActivity): number[] {
  const stream = activity.details.streams.find((item) => item.streamType === 'watts');
  if (!stream || !Array.isArray(stream.data)) {
    return [];
  }

  return stream.data.flatMap((value) => (typeof value === 'number' ? [Math.round(value)] : []));
}

function normalizeWidthUnits(durationSeconds: number | null | undefined): number {
  if (!durationSeconds || durationSeconds <= 0) {
    return 1;
  }

  return durationSeconds;
}

function completedIntervalDurationSeconds(interval: IntervalActivity['details']['intervals'][number]): number {
  const inferredDuration = interval.startTimeSeconds !== null && interval.endTimeSeconds !== null
    ? interval.endTimeSeconds - interval.startTimeSeconds
    : null;

  return normalizeWidthUnits(interval.movingTimeSeconds ?? interval.elapsedTimeSeconds ?? inferredDuration);
}

function matchedIntervalDurationSeconds(interval: NonNullable<IntervalEvent['actualWorkout']>['matchedIntervals'][number]): number {
  const inferredDuration = interval.actualStartTimeSeconds !== null && interval.actualEndTimeSeconds !== null
    ? interval.actualEndTimeSeconds - interval.actualStartTimeSeconds
    : null;

  return normalizeWidthUnits(inferredDuration ?? interval.plannedDurationSeconds);
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
