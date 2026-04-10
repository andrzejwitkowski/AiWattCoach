import type { IntervalActivity, IntervalEvent } from '../intervals/types';
import type { CalendarDay, CalendarRaceLabel } from './types';
import { formatRaceSubtitle } from './racePresentation';
import type { WorkoutDetailSelection } from './workoutDetails';
import { isPlannedWorkoutEvent } from './workoutDetails';

type BuildDayItemsOptions = {
  locale: string;
  labels: {
    plannedWorkout: string;
    workout: string;
  };
  t: (key: string, options?: Record<string, unknown>) => string;
};

export type CalendarDayItem =
  | {
    kind: 'race';
    id: string;
    title: string;
    subtitle: string | null;
    dateKey: string;
    race: CalendarRaceLabel;
    priorityRank: number;
    tss: number | null;
  }
  | {
    kind: 'planned';
    id: string;
    title: string;
    subtitle: string | null;
    dateKey: string;
    event: IntervalEvent;
    priorityRank: number;
    tss: number | null;
  }
  | {
    kind: 'completed';
    id: string;
    title: string;
    subtitle: string | null;
    dateKey: string;
    activity: IntervalActivity;
    event: IntervalEvent | null;
    priorityRank: number;
    tss: number | null;
  }
  | {
    kind: 'event';
    id: string;
    title: string;
    subtitle: string | null;
    dateKey: string;
    event: IntervalEvent;
    priorityRank: number;
    tss: number | null;
  };

export type CalendarDayItemsSelection = {
  dateKey: string;
  items: CalendarDayItem[];
};

export function buildDayItems(day: CalendarDay, options: BuildDayItemsOptions): CalendarDayItem[] {
  const items: CalendarDayItem[] = [];
  const linkedRaceEventId = day.labels.find((label): label is CalendarRaceLabel => label.kind === 'race')?.payload.linkedIntervalsEventId ?? null;

  for (const label of day.labels) {
    if (label.kind !== 'race') {
      continue;
    }

      items.push({
        kind: 'race',
        id: `race:${label.payload.raceId}`,
        title: label.payload.name,
        subtitle: formatRaceSubtitle(label.payload, options.t),
        dateKey: day.dateKey,
        race: label,
        priorityRank: 0,
      tss: null,
    });
  }

  for (const event of day.events) {
    const normalizedEventId = event.linkedIntervalsEventId ?? event.id;
    const plannedWorkout = isPlannedWorkoutEvent(event);

    if (linkedRaceEventId !== null && normalizedEventId === linkedRaceEventId) {
      continue;
    }

    if (event.actualWorkout) {
      continue;
    }

    if (plannedWorkout) {
      items.push({
        kind: 'planned',
        id: `planned:${event.calendarEntryId ?? event.id}`,
        title: event.name ?? options.labels.plannedWorkout,
        subtitle: summarizePlannedEvent(event, options.locale),
        dateKey: day.dateKey,
        event,
        priorityRank: 1,
        tss: event.eventDefinition.summary.estimatedTrainingStressScore !== null
          ? Math.round(event.eventDefinition.summary.estimatedTrainingStressScore)
          : null,
      });
      continue;
    }

    items.push(buildGenericEventItem(event, day.dateKey));
  }

  for (const activity of day.activities) {
    const matchedEvent = day.events.find((event) => event.actualWorkout?.activityId === activity.id) ?? null;
    items.push({
      kind: 'completed',
      id: `activity:${activity.id}`,
      title: activity.name ?? activity.activityType ?? options.labels.workout,
      subtitle: summarizeCompletedActivity(activity, options.locale),
      dateKey: day.dateKey,
      activity,
      event: matchedEvent,
      priorityRank: 1,
      tss: activity.metrics.trainingStressScore,
    });
  }

  return items.sort(compareDayItems);
}

export function selectDayItemDetail(item: CalendarDayItem): WorkoutDetailSelection | null {
  switch (item.kind) {
    case 'planned':
      return {
        dateKey: item.dateKey,
        event: item.event,
        activity: null,
      };
    case 'completed':
      return {
        dateKey: item.dateKey,
        event: item.event,
        activity: item.activity,
      };
    case 'event':
      return {
        dateKey: item.dateKey,
        event: item.event,
        activity: null,
      };
    case 'race':
      return null;
  }
}

function compareDayItems(left: CalendarDayItem, right: CalendarDayItem): number {
  if (left.priorityRank !== right.priorityRank) {
    return left.priorityRank - right.priorityRank;
  }

  const leftTss = left.tss ?? -1;
  const rightTss = right.tss ?? -1;
  if (leftTss !== rightTss) {
    return rightTss - leftTss;
  }

  return left.title.localeCompare(right.title);
}

function summarizePlannedEvent(event: IntervalEvent, locale: string): string | null {
  const durationMinutes = event.eventDefinition.summary.totalDurationSeconds > 0
    ? Math.round(event.eventDefinition.summary.totalDurationSeconds / 60)
    : null;
  const tss = event.eventDefinition.summary.estimatedTrainingStressScore !== null
    ? Math.round(event.eventDefinition.summary.estimatedTrainingStressScore)
    : null;

  const durationLabel = formatMinutes(durationMinutes, locale);

  if (durationLabel && tss !== null) {
    return `${durationLabel} • ${tss} TSS`;
  }

  if (durationLabel) {
    return durationLabel;
  }

  return tss !== null ? `${tss} TSS` : null;
}

function summarizeCompletedActivity(activity: IntervalActivity, locale: string): string | null {
  const durationSeconds = activity.movingTimeSeconds ?? activity.elapsedTimeSeconds;
  const durationMinutes = durationSeconds && durationSeconds > 0
    ? Math.round(durationSeconds / 60)
    : null;
  const tss = activity.metrics.trainingStressScore;
  const durationLabel = formatMinutes(durationMinutes, locale);

  if (durationLabel && tss !== null) {
    return `${durationLabel} • ${tss} TSS`;
  }

  if (durationLabel) {
    return durationLabel;
  }

  return tss !== null ? `${tss} TSS` : null;
}

function buildGenericEventItem(event: IntervalEvent, dateKey: string): Extract<CalendarDayItem, { kind: 'event' }> {
  return {
    kind: 'event',
    id: `event:${event.calendarEntryId ?? event.id}`,
    title: event.name ?? event.category,
    subtitle: null,
    dateKey,
    event,
    priorityRank: 3,
    tss: null,
  };
}

function formatMinutes(value: number | null, locale: string): string | null {
  if (value === null) {
    return null;
  }

  return new Intl.NumberFormat(locale, {
    style: 'unit',
    unit: 'minute',
    unitDisplay: 'short',
    maximumFractionDigits: 0,
  }).format(value);
}
