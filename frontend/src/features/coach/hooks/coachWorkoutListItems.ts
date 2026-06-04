import { addDays, extractDateKey, getMondayOfWeek, toDateKey } from '../../calendar/utils/dateUtils';
import type { IntervalActivity, IntervalEvent } from '../../intervals/types';
import type { CoachWorkoutListItem, WorkoutSummary } from '../types';
import type { WorkoutSummaryDateRange } from '../api/workoutSummary';

export function formatRangeLabel(startDate: Date, endDate: Date): string {
  const formatter = new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
  });
  return `${formatter.format(startDate)} - ${formatter.format(endDate)}`;
}

function normalizeName(value: string | null | undefined): string {
  return value?.trim().toLowerCase().replace(/\s+/g, ' ') ?? '';
}

function namesLookRelated(event: IntervalEvent, activity: IntervalActivity): boolean {
  const eventName = normalizeName(event.name);
  const activityName = normalizeName(activity.name) || normalizeName(activity.activityType);

  if (!eventName || !activityName) {
    return false;
  }

  return eventName === activityName || eventName.includes(activityName) || activityName.includes(eventName);
}

function chooseMatchedActivity(
  event: IntervalEvent,
  candidates: IntervalActivity[],
  dayEventCount: number,
  dayActivityCount: number,
): IntervalActivity | null {
  const namedCandidates = candidates.filter((activity) => namesLookRelated(event, activity));

  if (namedCandidates.length === 1) {
    return namedCandidates[0] ?? null;
  }

  if (candidates.length === 1 && dayEventCount === 1 && dayActivityCount === 1) {
    return candidates[0] ?? null;
  }

  return null;
}

function inferEventIdHint(activity: IntervalActivity): string | null {
  const values = [activity.externalId, activity.description, activity.name];

  for (const value of values) {
    const match = value?.match(/paired_event_id=(\d+)/i);
    if (match?.[1]) {
      return match[1];
    }
  }

  return null;
}

export function buildWorkoutItems(
  events: IntervalEvent[],
  activities: IntervalActivity[],
): CoachWorkoutListItem[] {
  const eventsSorted = [...events].sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal));
  const activitiesSorted = [...activities].sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal));
  const activitiesByDate = new Map<string, IntervalActivity[]>();
  const eventsByDate = new Map<string, IntervalEvent[]>();
  const eventsByActualWorkoutActivityId = new Map<string, IntervalEvent[]>();

  for (const activity of activitiesSorted) {
    const dateKey = extractDateKey(activity.startDateLocal);
    const existing = activitiesByDate.get(dateKey) ?? [];
    existing.push(activity);
    activitiesByDate.set(dateKey, existing);
  }

  for (const event of eventsSorted) {
    const dateKey = extractDateKey(event.startDateLocal);
    const existing = eventsByDate.get(dateKey) ?? [];
    existing.push(event);
    eventsByDate.set(dateKey, existing);

    const actualWorkoutActivityId = event.actualWorkout?.activityId;
    if (actualWorkoutActivityId) {
      const activityMatches = eventsByActualWorkoutActivityId.get(actualWorkoutActivityId) ?? [];
      activityMatches.push(event);
      eventsByActualWorkoutActivityId.set(actualWorkoutActivityId, activityMatches);
    }
  }

  const matchedActivityIds = new Set<string>();
  const matchedEventIds = new Set<number>();
  const activityEventMatches = new Map<string, IntervalEvent>();

  for (const activity of activitiesSorted) {
    const linkedEvent = (eventsByActualWorkoutActivityId.get(activity.id) ?? []).find(
      (event) => !matchedEventIds.has(event.id),
    );
    if (!linkedEvent || matchedActivityIds.has(activity.id)) {
      continue;
    }

    matchedActivityIds.add(activity.id);
    matchedEventIds.add(linkedEvent.id);
    activityEventMatches.set(activity.id, linkedEvent);
  }

  for (const activity of activitiesSorted) {
    const hintedEventId = inferEventIdHint(activity);
    if (!hintedEventId) {
      continue;
    }

    const hintedEvent = eventsSorted.find((event) => String(event.id) === hintedEventId);
    if (!hintedEvent || matchedActivityIds.has(activity.id) || matchedEventIds.has(hintedEvent.id)) {
      continue;
    }

    matchedActivityIds.add(activity.id);
    matchedEventIds.add(hintedEvent.id);
    activityEventMatches.set(activity.id, hintedEvent);
  }

  for (const event of eventsSorted) {
    if (matchedEventIds.has(event.id)) {
      continue;
    }

    const dateKey = extractDateKey(event.startDateLocal);
    const candidates = (activitiesByDate.get(dateKey) ?? []).filter((activity) => !matchedActivityIds.has(activity.id));
    const matchedActivity = chooseMatchedActivity(
      event,
      candidates,
      (eventsByDate.get(dateKey) ?? []).length,
      (activitiesByDate.get(dateKey) ?? []).length,
    );

    if (!matchedActivity) {
      continue;
    }

    matchedActivityIds.add(matchedActivity.id);
    matchedEventIds.add(event.id);
    activityEventMatches.set(matchedActivity.id, event);
  }

  const items: CoachWorkoutListItem[] = activitiesSorted.map((activity) => {
    const matchedEvent = activityEventMatches.get(activity.id) ?? null;

    return {
      id: activity.id,
      source: 'activity',
      startDateLocal: activity.startDateLocal,
      event: matchedEvent,
      activity,
      summary: null,
      hasSummary: false,
      hasConversation: false,
    };
  });

  for (const event of eventsSorted) {
    if (matchedEventIds.has(event.id)) {
      continue;
    }

    items.push({
      id: String(event.id),
      source: 'event',
      startDateLocal: event.startDateLocal,
      event,
      activity: null,
      summary: null,
      hasSummary: false,
      hasConversation: false,
    });
  }

  return items.sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal));
}

export function isSameDay(left: Date, right: Date): boolean {
  return (
    left.getFullYear() === right.getFullYear()
    && left.getMonth() === right.getMonth()
    && left.getDate() === right.getDate()
  );
}

function isWithinWeek(value: string, weekStart: Date): boolean {
  const weekStartKey = toDateKey(weekStart);
  const weekEndKey = toDateKey(addDays(weekStart, 6));
  const dateKey = extractDateKey(value);

  return dateKey >= weekStartKey && dateKey <= weekEndKey;
}

function withSummaryState(item: CoachWorkoutListItem, summary: WorkoutSummary | null): CoachWorkoutListItem {
  return {
    ...item,
    summary,
    hasSummary: summary !== null,
    hasConversation:
      summary?.hasCoachMessage
      ?? summary?.messages.some((message) => message.role === 'coach')
      ?? false,
  };
}

export function weekDateRange(weekStart: Date): WorkoutSummaryDateRange {
  return {
    oldest: toDateKey(weekStart),
    newest: toDateKey(addDays(weekStart, 6)),
  };
}

export function buildVisibleItems(
  allItems: CoachWorkoutListItem[],
  visibleWeekStart: Date,
  getSummary: (workoutId: string) => WorkoutSummary | undefined,
): CoachWorkoutListItem[] {
  const todayDateKey = toDateKey(new Date());

  return allItems
    .filter(
      (item) =>
        item.source === 'activity'
        && extractDateKey(item.startDateLocal) <= todayDateKey
        && isWithinWeek(item.startDateLocal, visibleWeekStart),
    )
    .map((item) => withSummaryState(item, getSummary(item.id) ?? null));
}

export function chunkWorkoutIds(workoutIds: string[], maxBatchSize: number): string[][] {
  const chunks: string[][] = [];

  for (let index = 0; index < workoutIds.length; index += maxBatchSize) {
    chunks.push(workoutIds.slice(index, index + maxBatchSize));
  }

  return chunks;
}

export function defaultVisibleWeekStart(items: CoachWorkoutListItem[], currentWeekStart: Date): Date {
  if (items.some((item) => isWithinWeek(item.startDateLocal, currentWeekStart))) {
    return currentWeekStart;
  }

  const newestItem = items[0];
  return newestItem ? getMondayOfWeek(new Date(newestItem.startDateLocal)) : currentWeekStart;
}
