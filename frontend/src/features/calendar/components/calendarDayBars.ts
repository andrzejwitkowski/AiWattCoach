import type { CalendarDay } from '../types';
import {
  buildCompletedWorkoutPreviewBars,
  buildMatchedWorkoutBars,
  buildPlannedWorkoutBars,
  type WorkoutBar,
} from '../workoutDetails';

export function buildBars(
  dayActivity: CalendarDay['activities'][number] | null,
  dayEvent: CalendarDay['events'][number] | null,
): Array<number | WorkoutBar> {
  if (dayEvent?.restDay) {
    return [18, 12, 20];
  }

  if (dayEvent?.actualWorkout?.matchedIntervals.length) {
    const bars = buildMatchedWorkoutBars(dayEvent.actualWorkout);
    if (bars.length > 0) {
      return bars;
    }
  }

  if (dayActivity) {
    const bars = buildCompletedWorkoutPreviewBars(dayActivity);
    if (bars.length > 0) {
      return bars;
    }
  }

  if (dayEvent) {
    const bars = buildPlannedWorkoutBars(dayEvent);
    if (bars.length > 0) {
      return bars;
    }
  }

  const tss = dayActivity?.metrics.trainingStressScore ?? 0;
  if (tss > 0) {
    const peak = Math.min(100, Math.max(30, tss));
    return [Math.max(20, peak - 25), peak, Math.max(25, peak - 10)];
  }

  return [35, 55, 75, 55];
}
