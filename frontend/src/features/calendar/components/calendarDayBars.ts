import type { CalendarDay } from '../types';
import {
  buildCompletedWorkoutPreviewBars,
  buildMatchedWorkoutBars,
  buildPlannedWorkoutBars,
  buildPowerTraceSeries,
} from '../workoutDetails';
import type {CalendarMiniChartData} from './CalendarMiniChart';

export function buildBars(
  dayActivity: CalendarDay['activities'][number] | null,
  dayEvent: CalendarDay['events'][number] | null,
): CalendarMiniChartData {
  if (dayEvent?.restDay) {
    return {kind: 'bars', bars: [18, 12, 20]};
  }

  const powerTrace = buildPowerTraceSeries(dayActivity, dayEvent?.actualWorkout?.powerValues ?? null);
  if (powerTrace) {
    return {kind: 'power-trace', values: powerTrace.rawValues, ftpWatts: powerTrace.ftpWatts};
  }

  if (dayEvent?.actualWorkout?.matchedIntervals.length) {
    const bars = buildMatchedWorkoutBars(dayEvent.actualWorkout);
    if (bars.length > 0) {
      return {kind: 'bars', bars};
    }
  }

  if (dayActivity) {
    const bars = buildCompletedWorkoutPreviewBars(dayActivity);
    if (bars.length > 0) {
      return {kind: 'bars', bars};
    }
  }

  if (dayEvent) {
    const bars = buildPlannedWorkoutBars(dayEvent);
    if (bars.length > 0) {
      return {kind: 'bars', bars};
    }
  }

  const tss = dayActivity?.metrics.trainingStressScore ?? 0;
  if (tss > 0) {
    const peak = Math.min(100, Math.max(30, tss));
    return {kind: 'bars', bars: [Math.max(20, peak - 25), peak, Math.max(25, peak - 10)]};
  }

  return {kind: 'bars', bars: [35, 55, 75, 55]};
}
