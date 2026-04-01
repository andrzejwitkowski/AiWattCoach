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

export function buildCompletedWorkoutPreviewBars(activity: IntervalActivity): WorkoutBar[] {
  const skylineBars = buildSkylineChartBars(activity.details.skylineChart);
  if (skylineBars.length > 0) {
    return skylineBars;
  }

  return buildCompletedWorkoutBars(activity);
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
  const selectedEvent = matchedActivity ? event : null;

  return {
    dateKey,
    event: selectedEvent,
    activity: selectedEvent ? matchedActivity : activities[0] ?? null,
  };
}

export function formatDurationLabel(totalSeconds: number | null | undefined): string {
  if (!totalSeconds || totalSeconds <= 0) {
    return '0m';
  }

  if (totalSeconds < 60) {
    return `${Math.floor(totalSeconds)}s`;
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

export function buildFiveSecondAveragePowerSeries(values: number[]): number[] {
  if (values.length === 0) {
    return [];
  }

  const averaged: number[] = [];
  let rollingSum = 0;

  for (let index = 0; index < values.length; index += 1) {
    rollingSum += values[index];

    if (index >= 5) {
      rollingSum -= values[index - 5];
    }

    const windowSize = Math.min(index + 1, 5);
    averaged.push(Math.round(rollingSum / windowSize));
  }

  return averaged;
}

function normalizeWidthUnits(durationSeconds: number | null | undefined): number {
  if (!durationSeconds || durationSeconds <= 0) {
    return 1;
  }

  return durationSeconds;
}

function buildSkylineChartBars(encodedCharts: string[]): WorkoutBar[] {
  for (const encodedChart of encodedCharts) {
    const decodedBars = decodeSkylineChartBars(encodedChart);
    if (decodedBars.length > 0) {
      return decodedBars;
    }
  }

  return [];
}

function decodeSkylineChartBars(encodedChart: string): WorkoutBar[] {
  if (!encodedChart.trim()) {
    return [];
  }

  let bytes: Uint8Array;
  try {
    bytes = decodeBase64Bytes(encodedChart);
  } catch {
    return [];
  }

  const chart = parseSkylineChart(bytes);
  const barCount = Math.max(chart.width.length, chart.intensity.length, chart.zone.length);
  if (barCount === 0) {
    return [];
  }

  const normalizedWidths = normalizeSkylineWidths(chart.width, barCount);

  return Array.from({length: barCount}, (_, index) => ({
    height: heightForPercent(chart.intensity[index] ?? 25),
    color: POWER_ZONE_COLORS[chart.zone[index] ?? 4] ?? POWER_ZONE_COLORS[4],
    widthUnits: normalizedWidths[index] ?? 1,
  }));
}

function decodeBase64Bytes(value: string): Uint8Array {
  const decoded = globalThis.atob(value);
  return Uint8Array.from(decoded, (character) => character.charCodeAt(0));
}

function parseSkylineChart(bytes: Uint8Array): {
  width: number[];
  intensity: number[];
  zone: number[];
} {
  const width: number[] = [];
  const intensity: number[] = [];
  const zone: number[] = [];
  let offset = 0;

  while (offset < bytes.length) {
    const key = readVarint(bytes, offset);
    if (key === null) {
      break;
    }

    offset = key.nextOffset;
    const fieldNumber = key.value >> 3;
    const wireType = key.value & 0x07;

    if (fieldNumber === 2 || fieldNumber === 3 || fieldNumber === 4) {
      const values = wireType === 2
        ? readPackedVarints(bytes, offset)
        : readSingleVarint(bytes, offset);
      if (values === null) {
        break;
      }

      offset = values.nextOffset;
      const target = fieldNumber === 2 ? width : fieldNumber === 3 ? intensity : zone;
      target.push(...values.values);
      continue;
    }

    const skipped = skipField(bytes, offset, wireType);
    if (skipped === null) {
      break;
    }

    offset = skipped;
  }

  return {width, intensity, zone};
}

function readPackedVarints(bytes: Uint8Array, offset: number): { values: number[]; nextOffset: number } | null {
  const length = readVarint(bytes, offset);
  if (length === null) {
    return null;
  }

  offset = length.nextOffset;
  const endOffset = offset + length.value;
  if (endOffset > bytes.length) {
    return null;
  }

  const values: number[] = [];
  while (offset < endOffset) {
    const item = readVarint(bytes, offset);
    if (item === null) {
      return null;
    }

    values.push(item.value);
    offset = item.nextOffset;
  }

  return {values, nextOffset: offset};
}

function readSingleVarint(bytes: Uint8Array, offset: number): { values: number[]; nextOffset: number } | null {
  const item = readVarint(bytes, offset);
  if (item === null) {
    return null;
  }

  return {values: [item.value], nextOffset: item.nextOffset};
}

function skipField(bytes: Uint8Array, offset: number, wireType: number): number | null {
  if (wireType === 0) {
    const value = readVarint(bytes, offset);
    return value?.nextOffset ?? null;
  }

  if (wireType === 2) {
    const length = readVarint(bytes, offset);
    if (length === null) {
      return null;
    }

    const nextOffset = length.nextOffset + length.value;
    return nextOffset <= bytes.length ? nextOffset : null;
  }

  return null;
}

function readVarint(bytes: Uint8Array, offset: number): { value: number; nextOffset: number } | null {
  let value = 0;
  let shift = 0;

  while (offset < bytes.length) {
    const byte = bytes[offset];
    value |= (byte & 0x7f) << shift;
    offset += 1;

    if ((byte & 0x80) === 0) {
      return {value, nextOffset: offset};
    }

    shift += 7;
    if (shift > 28) {
      return null;
    }
  }

  return null;
}

function normalizeSkylineWidths(widths: number[], barCount: number): number[] {
  if (barCount === 0) {
    return [];
  }

  const rawWidths = Array.from({length: barCount}, (_, index) => widths[index] ?? 1);
  const maxWidth = Math.max(...rawWidths, 1);

  return rawWidths.map((width) => {
    const normalized = maxWidth > 512 ? Math.round(width / 109) : width;
    return Math.max(1, normalized);
  });
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
