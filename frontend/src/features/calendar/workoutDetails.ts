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

export type PlannedWorkoutStructureItem = {
  id: string;
  label: string;
  detail: string | null;
  durationSeconds: number | null;
};

export type PlannedWorkoutChartInterval = {
  id: string;
  startSecond: number;
  endSecond: number;
  label: string;
};

type PlannedIntervalDefinition = IntervalEvent['eventDefinition']['intervals'][number];
type PlannedWorkoutSegment = IntervalEvent['eventDefinition']['segments'][number];

const POWER_ZONE_COLORS: Record<number, string> = {
  1: '#6b7280',
  2: '#00e3fd',
  3: '#52c41a',
  4: '#d2ff9a',
  5: '#facc15',
  6: '#ff7351',
  7: '#800020',
};

const DEFAULT_ZONE_TARGET_PERCENT: Record<number, number> = {
  1: 55,
  2: 70,
  3: 85,
  4: 100,
  5: 115,
  6: 130,
  7: 150,
};

export function isPlannedWorkoutEvent(
  event: IntervalEvent | null | undefined,
): event is IntervalEvent {
  if (!event || event.category !== 'WORKOUT') {
    return false;
  }

  return event.eventDefinition.intervals.length > 0
    || event.eventDefinition.segments.length > 0
    || event.eventDefinition.summary.totalDurationSeconds > 0
    || event.eventDefinition.summary.totalSegments > 0
    || Boolean(event.eventDefinition.rawWorkoutDoc?.trim());
}

export function buildPlannedWorkoutBars(event: IntervalEvent): WorkoutBar[] {
  const expandedSegments = buildExpandedPlannedSegments(event);
  if (expandedSegments.length === 0) {
    return event.eventDefinition.intervals.map((interval) => ({
      height: heightForPercent(plannedTargetValue(interval.targetPercentFtp, interval.zoneId)),
      color: POWER_ZONE_COLORS[interval.zoneId ?? 2] ?? POWER_ZONE_COLORS[2],
      widthUnits: plannedIntervalTotalDurationSeconds(interval),
    }));
  }

  return expandedSegments.map((segment) => ({
    height: heightForPercent(plannedTargetValue(segment.targetPercentFtp, segment.zoneId)),
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

export function buildPlannedWorkoutPowerSeries(
  event: IntervalEvent,
  sampleDurationSeconds = 5,
): number[] {
  const segments = buildExpandedPlannedSegments(event);

  if (segments.length > 0) {
    return segments.flatMap((segment) =>
      Array.from(
        {
          length: plannedSampleCount(segment.durationSeconds, sampleDurationSeconds),
        },
        () => plannedTargetValue(segment.targetPercentFtp, segment.zoneId),
      ),
    );
  }

  return event.eventDefinition.intervals.flatMap((interval) =>
    Array.from(
      {
        length: plannedSampleCount(plannedIntervalTotalDurationSeconds(interval), sampleDurationSeconds),
      },
      () => plannedTargetValue(interval.targetPercentFtp, interval.zoneId),
    ),
  );
}

export function buildPlannedWorkoutChartIntervals(
  event: IntervalEvent,
): PlannedWorkoutChartInterval[] {
  const segments = buildExpandedPlannedSegments(event);

  if (segments.length > 0) {
    return segments.map((segment) => ({
      id: `planned-${event.id}-${segment.order}`,
      startSecond: segment.startOffsetSeconds,
      endSecond: segment.endOffsetSeconds,
      label: segment.label,
    }));
  }

  let currentOffset = 0;

  return event.eventDefinition.intervals.map((interval, index) => {
    const durationSeconds = plannedIntervalTotalDurationSeconds(interval);
    const chartInterval = {
      id: `planned-${event.id}-interval-${index}`,
      startSecond: currentOffset,
      endSecond: currentOffset + durationSeconds,
      label: formatPlannedWorkoutIntervalLabel(interval),
    };
    currentOffset += durationSeconds;
    return chartInterval;
  });
}

export function buildPlannedWorkoutStructureItems(
  event: IntervalEvent,
): PlannedWorkoutStructureItem[] {
  if (event.eventDefinition.intervals.length > 0) {
    return event.eventDefinition.intervals.map((interval, index) => ({
      id: `interval-${index}`,
      label: formatPlannedWorkoutIntervalLabel(interval),
      detail: buildPlannedWorkoutIntervalDetail(interval),
      durationSeconds: plannedIntervalTotalDurationSeconds(interval),
    }));
  }

  const segments = buildExpandedPlannedSegments(event);

  if (segments.length > 0) {
    return segments.map((segment) => ({
      id: `segment-${segment.order}`,
      label: segment.label,
      detail: buildSegmentDetail(segment),
      durationSeconds: segment.durationSeconds,
    }));
  }

  const rawWorkoutDoc = event.eventDefinition.rawWorkoutDoc?.trim();

  if (!rawWorkoutDoc) {
    return [];
  }

  return rawWorkoutDoc
    .split('\n')
    .map((line) => normalizeWorkoutText(line))
    .filter(Boolean)
    .map((label, index) => ({
      id: `raw-${index}`,
      label,
      detail: null,
      durationSeconds: null,
    }));
}

export function formatPlannedWorkoutIntervalLabel(
  interval: PlannedIntervalDefinition,
): string {
  const definition = normalizeWorkoutText(interval.definition);

  if (definition) {
    return formatDefinitionLabel(definition, interval.repeatCount);
  }

  const durationLabel = formatDurationLabel(interval.durationSeconds);
  const targetLabel = buildPlannedTargetLabel(
    interval.targetPercentFtp,
    interval.zoneId,
  );
  const baseLabel = targetLabel ? `${durationLabel} @ ${targetLabel}` : durationLabel;

  return interval.repeatCount > 1
    ? `${interval.repeatCount} x ${baseLabel}`
    : baseLabel;
}

export function buildPlannedTargetLabel(
  targetPercentFtp: number | null | undefined,
  zoneId: number | null | undefined,
): string | null {
  if (targetPercentFtp !== null && targetPercentFtp !== undefined && targetPercentFtp > 0) {
    return `${trimTrailingZeros(targetPercentFtp)}% FTP`;
  }

  const fallbackPercent = zoneId ? DEFAULT_ZONE_TARGET_PERCENT[zoneId] : null;
  return fallbackPercent ? `Z${zoneId} target` : null;
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

  if (activities.length === 0) {
    return {
      dateKey,
      event: event?.actualWorkout || isPlannedWorkoutEvent(event) ? event : null,
      activity: null,
    };
  }

  const matchedActivity = event?.actualWorkout?.activityId
    ? activities.find((candidate) => candidate.id === event.actualWorkout?.activityId) ?? null
    : null;

  if (matchedActivity) {
    return {
      dateKey,
      event,
      activity: matchedActivity,
    };
  }

  if (event?.actualWorkout) {
    return {
      dateKey,
      event: null,
      activity: activities[0] ?? null,
    };
  }

  return {
    dateKey,
    event: isPlannedWorkoutEvent(event) ? event : null,
    activity: activities[0] ?? null,
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

function buildPlannedWorkoutIntervalDetail(
  interval: PlannedIntervalDefinition,
): string | null {
  if (interval.repeatCount > 1) {
    return formatDurationLabel(plannedIntervalTotalDurationSeconds(interval));
  }

  const detailParts = [
    interval.durationSeconds ? formatDurationLabel(interval.durationSeconds) : null,
    buildPlannedTargetLabel(interval.targetPercentFtp, interval.zoneId),
  ].filter(Boolean);

  return detailParts.length > 0 ? detailParts.join(' • ') : null;
}

function buildExpandedPlannedSegments(
  event: IntervalEvent,
): PlannedWorkoutSegment[] {
  const segments = event.eventDefinition.segments ?? [];

  if (segments.length === 0) {
    return [];
  }

  const summaryDuration = event.eventDefinition.summary.totalDurationSeconds;
  const segmentDuration = segments.reduce((total, segment) => total + segment.durationSeconds, 0);
  if (summaryDuration <= 0 || segmentDuration <= 0 || segmentDuration >= summaryDuration) {
    return segments;
  }

  const repeatCount = inferSegmentRepeatCount(event, segmentDuration, summaryDuration);
  if (repeatCount <= 1) {
    return segments;
  }

  return Array.from({ length: repeatCount }, (_, repeatIndex) => {
    const repeatOffset = repeatIndex * segmentDuration;

    return segments.map((segment, segmentIndex) => ({
      ...segment,
      order: (repeatIndex * segments.length) + segmentIndex,
      label: repeatCount > 1 ? `${segment.label} ${repeatIndex + 1}` : segment.label,
      startOffsetSeconds: segment.startOffsetSeconds + repeatOffset,
      endOffsetSeconds: segment.endOffsetSeconds + repeatOffset,
    }));
  }).flat();
}

function inferSegmentRepeatCount(
  event: IntervalEvent,
  segmentDuration: number,
  summaryDuration: number,
): number {
  const intervalRepeatCount = event.eventDefinition.intervals.reduce(
    (maxRepeatCount, interval) => Math.max(maxRepeatCount, interval.repeatCount),
    1,
  );

  const repeatedDuration = segmentDuration * intervalRepeatCount;
  if (repeatedDuration === summaryDuration) {
    return intervalRepeatCount;
  }

  const inferredRepeatCount = Math.round(summaryDuration / segmentDuration);
  return inferredRepeatCount > 1 && (segmentDuration * inferredRepeatCount) === summaryDuration
    ? inferredRepeatCount
    : 1;
}

function plannedIntervalTotalDurationSeconds(
  interval: PlannedIntervalDefinition,
): number {
  return normalizeWidthUnits(interval.durationSeconds) * Math.max(1, interval.repeatCount);
}

function buildSegmentDetail(segment: PlannedWorkoutSegment): string | null {
  const detailParts = [
    formatDurationLabel(segment.durationSeconds),
    buildPlannedTargetLabel(segment.targetPercentFtp, segment.zoneId),
  ].filter(Boolean);

  return detailParts.length > 0 ? detailParts.join(' • ') : null;
}

function plannedSampleCount(
  durationSeconds: number | null | undefined,
  sampleDurationSeconds: number,
): number {
  const normalizedDuration = normalizeWidthUnits(durationSeconds);
  return Math.max(1, Math.ceil(normalizedDuration / Math.max(1, sampleDurationSeconds)));
}

function plannedTargetValue(
  targetPercentFtp: number | null | undefined,
  zoneId: number | null | undefined,
): number {
  if (targetPercentFtp !== null && targetPercentFtp !== undefined && targetPercentFtp > 0) {
    return Math.round(targetPercentFtp);
  }

  return zoneId ? DEFAULT_ZONE_TARGET_PERCENT[zoneId] ?? DEFAULT_ZONE_TARGET_PERCENT[4] : DEFAULT_ZONE_TARGET_PERCENT[4];
}

function formatDefinitionLabel(definition: string, repeatCount: number): string {
  let label = definition
    .replace(/\b(\d+)x(?=\d)/gi, '$1 x ')
    .replace(/(\d+(?:\.\d+)?)%\s*ftp\b/gi, '$1% FTP')
    .replace(/(\d+(?:\.\d+)?)%(?!\s*FTP\b)/g, '$1% FTP')
    .replace(/\s+/g, ' ')
    .trim();

  if (repeatCount > 1 && shouldWrapRepeatBlock(label, repeatCount)) {
    label = `${repeatCount} x (${label})`;
  }

  return label;
}

function normalizeWorkoutText(value: string | null | undefined): string {
  return (value ?? '').replace(/^[-*]\s*/, '').replace(/\s+/g, ' ').trim();
}

function startsWithRepeat(label: string, repeatCount: number): boolean {
  return new RegExp(`^${repeatCount}\\s*x\\b`, 'i').test(label);
}

function shouldWrapRepeatBlock(label: string, repeatCount: number): boolean {
  return !startsWithRepeat(label, repeatCount);
}

function trimTrailingZeros(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(2).replace(/\.0+$/, '').replace(/(\.\d*[1-9])0+$/, '$1');
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
