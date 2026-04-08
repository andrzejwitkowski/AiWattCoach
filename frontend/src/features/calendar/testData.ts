import type { IntervalActivity, IntervalEvent } from '../intervals/types';

import type { CalendarDay } from './types';
import type { WorkoutDetailSelection } from './workoutDetails';

type EventDefinitionOverrides = Partial<Omit<IntervalEvent['eventDefinition'], 'summary'>> & {
  summary?: Partial<IntervalEvent['eventDefinition']['summary']>;
};

type EventOverrides = Partial<Omit<IntervalEvent, 'eventDefinition' | 'actualWorkout'>> & {
  eventDefinition?: EventDefinitionOverrides;
  actualWorkout?: Partial<NonNullable<IntervalEvent['actualWorkout']>> | null;
};

type ActivityDetailsOverrides = Partial<Omit<IntervalActivity['details'], 'intervals' | 'intervalGroups' | 'streams'>> & {
  intervals?: IntervalActivity['details']['intervals'];
  intervalGroups?: IntervalActivity['details']['intervalGroups'];
  streams?: IntervalActivity['details']['streams'];
};

type ActivityOverrides = Partial<Omit<IntervalActivity, 'metrics' | 'details'>> & {
  metrics?: Partial<IntervalActivity['metrics']>;
  details?: ActivityDetailsOverrides;
};

export function makeWorkoutSummary(
  overrides: Partial<IntervalEvent['eventDefinition']['summary']> = {},
): IntervalEvent['eventDefinition']['summary'] {
  return {
    totalSegments: 0,
    totalDurationSeconds: 0,
    estimatedNormalizedPowerWatts: null,
    estimatedAveragePowerWatts: null,
    estimatedIntensityFactor: null,
    estimatedTrainingStressScore: null,
    ...overrides,
  };
}

export function makeIntervalDefinition(
  overrides: Partial<IntervalEvent['eventDefinition']['intervals'][number]> = {},
): IntervalEvent['eventDefinition']['intervals'][number] {
  return {
    definition: '10m endurance',
    repeatCount: 1,
    durationSeconds: null,
    targetPercentFtp: null,
    zoneId: null,
    ...overrides,
  };
}

export function makeWorkoutSegment(
  overrides: Partial<IntervalEvent['eventDefinition']['segments'][number]> = {},
): IntervalEvent['eventDefinition']['segments'][number] {
  return {
    order: 0,
    label: 'Segment',
    durationSeconds: 600,
    startOffsetSeconds: 0,
    endOffsetSeconds: 600,
    targetPercentFtp: null,
    zoneId: null,
    ...overrides,
  };
}

export function makeEventDefinition(overrides: EventDefinitionOverrides = {}): IntervalEvent['eventDefinition'] {
  return {
    rawWorkoutDoc: null,
    intervals: [],
    segments: [],
    ...overrides,
    summary: makeWorkoutSummary(overrides.summary),
  };
}

export function makeMatchedInterval(
  overrides: Partial<NonNullable<IntervalEvent['actualWorkout']>['matchedIntervals'][number]> = {},
): NonNullable<IntervalEvent['actualWorkout']>['matchedIntervals'][number] {
  return {
    plannedSegmentOrder: 0,
    plannedLabel: 'Segment',
    plannedDurationSeconds: 600,
    targetPercentFtp: null,
    zoneId: null,
    actualIntervalId: 1,
    actualStartTimeSeconds: 0,
    actualEndTimeSeconds: 600,
    averagePowerWatts: null,
    normalizedPowerWatts: null,
    averageHeartRateBpm: null,
    averageCadenceRpm: null,
    averageSpeedMps: null,
    complianceScore: 0.9,
    ...overrides,
  };
}

export function makeActualWorkout(
  overrides: Partial<NonNullable<IntervalEvent['actualWorkout']>> = {},
): NonNullable<IntervalEvent['actualWorkout']> {
  return {
    activityId: 'activity-1',
    activityName: 'Completed Ride',
    startDateLocal: '2026-03-25T08:00:00',
    powerValues: [],
    cadenceValues: [],
    heartRateValues: [],
    speedValues: [],
    averagePowerWatts: null,
    normalizedPowerWatts: null,
    trainingStressScore: null,
    intensityFactor: null,
    complianceScore: 0.9,
    matchedIntervals: [],
    ...overrides,
  };
}

export function makeEvent(overrides: EventOverrides = {}): IntervalEvent {
  const { eventDefinition, actualWorkout, ...eventOverrides } = overrides;

  return {
    id: 1,
    calendarEntryId: 'intervals:1',
    startDateLocal: '2026-03-25',
    name: 'Workout',
    category: 'WORKOUT',
    description: null,
    indoor: true,
    color: null,
    eventDefinition: makeEventDefinition(eventDefinition),
    actualWorkout:
      actualWorkout === undefined
        ? null
        : actualWorkout === null
          ? null
          : makeActualWorkout(actualWorkout),
    plannedSource: 'intervals',
    syncStatus: null,
    linkedIntervalsEventId: null,
    projectedWorkout: null,
    ...eventOverrides,
  };
}

export function makeActivityMetrics(
  overrides: Partial<IntervalActivity['metrics']> = {},
): IntervalActivity['metrics'] {
  return {
    trainingStressScore: null,
    normalizedPowerWatts: null,
    intensityFactor: null,
    efficiencyFactor: null,
    variabilityIndex: null,
    averagePowerWatts: null,
    ftpWatts: null,
    totalWorkJoules: null,
    calories: null,
    trimp: null,
    powerLoad: null,
    heartRateLoad: null,
    paceLoad: null,
    strainScore: null,
    ...overrides,
  };
}

export function makeActivityInterval(
  overrides: Partial<IntervalActivity['details']['intervals'][number]> = {},
): IntervalActivity['details']['intervals'][number] {
  return {
    id: 1,
    label: 'Ride 1',
    intervalType: 'WORK',
    groupId: null,
    startIndex: null,
    endIndex: null,
    startTimeSeconds: 0,
    endTimeSeconds: 600,
    movingTimeSeconds: 600,
    elapsedTimeSeconds: 600,
    distanceMeters: null,
    averagePowerWatts: null,
    normalizedPowerWatts: null,
    trainingStressScore: null,
    averageHeartRateBpm: null,
    averageCadenceRpm: null,
    averageSpeedMps: null,
    averageStrideMeters: null,
    zone: null,
    ...overrides,
  };
}

export function makeActivityStream(
  overrides: Partial<IntervalActivity['details']['streams'][number]> = {},
): IntervalActivity['details']['streams'][number] {
  return {
    streamType: 'watts',
    name: 'Power',
    data: [],
    data2: null,
    valueTypeIsArray: false,
    custom: false,
    allNull: false,
    ...overrides,
  };
}

export function makeActivityDetails(overrides: ActivityDetailsOverrides = {}): IntervalActivity['details'] {
  return {
    intervals: [],
    intervalGroups: [],
    streams: [],
    intervalSummary: [],
    skylineChart: [],
    powerZoneTimes: [],
    heartRateZoneTimes: [],
    paceZoneTimes: [],
    gapZoneTimes: [],
    ...overrides,
  };
}

export function makeActivity(overrides: ActivityOverrides = {}): IntervalActivity {
  const { metrics, details, ...activityOverrides } = overrides;

  return {
    id: 'activity-1',
    startDateLocal: '2026-03-25T08:00:00',
    startDate: '2026-03-25T07:00:00Z',
    name: 'Ride',
    description: null,
    activityType: 'Ride',
    source: null,
    externalId: null,
    deviceName: null,
    distanceMeters: null,
    movingTimeSeconds: null,
    elapsedTimeSeconds: null,
    totalElevationGainMeters: null,
    averageSpeedMps: null,
    averageHeartRateBpm: null,
    averageCadenceRpm: null,
    trainer: false,
    commute: false,
    race: false,
    hasHeartRate: false,
    streamTypes: [],
    tags: [],
    metrics: makeActivityMetrics(metrics),
    details: makeActivityDetails(details),
    detailsUnavailableReason: null,
    ...activityOverrides,
  };
}

export function makeCalendarDay(overrides: Partial<CalendarDay> = {}): CalendarDay {
  return {
    date: new Date(2026, 2, 25),
    dateKey: '2026-03-25',
    events: [],
    activities: [],
    ...overrides,
  };
}

export function makeSelection(
  overrides: Partial<WorkoutDetailSelection> = {},
): WorkoutDetailSelection {
  return {
    dateKey: '2026-03-25',
    event: null,
    activity: null,
    ...overrides,
  };
}
