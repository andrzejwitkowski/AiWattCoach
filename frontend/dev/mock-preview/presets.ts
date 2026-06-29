type DashboardRange = '90d' | 'season' | 'all-time';

type CurrentUser = {
  id: string;
  email: string;
  displayName: string | null;
  avatarUrl: string | null;
  roles: Array<'user' | 'admin'>;
};

type UserSettings = {
  aiAgents: {
    openaiApiKey: string | null;
    openaiApiKeySet: boolean;
    geminiApiKey: string | null;
    geminiApiKeySet: boolean;
    openrouterApiKey: string | null;
    openrouterApiKeySet: boolean;
    deepseekApiKey: string | null;
    deepseekApiKeySet: boolean;
    zaiApiKey: string | null;
    zaiApiKeySet: boolean;
    selectedProvider: 'openai' | 'gemini' | 'openrouter' | 'deepseek' | 'zai' | null;
    selectedModel: string | null;
  };
  intervals: {
    apiKey: string | null;
    apiKeySet: boolean;
    athleteId: string | null;
    connected: boolean;
  };
  wahoo: {
    available: boolean;
    accessToken: string | null;
    accessTokenSet: boolean;
    refreshTokenSet: boolean;
    expiresAtEpochSeconds: number | null;
    connected: boolean;
  };
  options: {
    analyzeWithoutHeartRate: boolean;
  };
  availability: {
    configured: boolean;
    days: AvailabilityDay[];
  };
  cycling: {
    fullName: string | null;
    age: number | null;
    heightCm: number | null;
    weightKg: number | null;
    ftpWatts: number | null;
    hrMaxBpm: number | null;
    vo2Max: number | null;
    athletePrompt: string | null;
    medications: string | null;
    athleteNotes: string | null;
    lastZoneUpdateEpochSeconds: number | null;
  };
};

type AvailabilityDay = {
  weekday: 'mon' | 'tue' | 'wed' | 'thu' | 'fri' | 'sat' | 'sun';
  available: boolean;
  maxDurationMinutes: number | null;
};

type DashboardPoint = {
  date: string;
  dailyTss: number | null;
  currentCtl: number | null;
  currentAtl: number | null;
  currentTsb: number | null;
};

type DashboardResponse = {
  range: DashboardRange;
  windowStart: string;
  windowEnd: string;
  hasTrainingLoad: boolean;
  summary: {
    currentCtl: number | null;
    currentAtl: number | null;
    currentTsb: number | null;
    ftpWatts: number | null;
    averageIf28d: number | null;
    averageEf28d: number | null;
    loadDeltaCtl14d: number | null;
    tsbZone: 'freshness_peak' | 'optimal_training' | 'high_risk';
  };
  points: DashboardPoint[];
};

type IntervalDefinition = {
  definition: string;
  repeatCount: number;
  durationSeconds: number | null;
  targetPercentFtp: number | null;
  zoneId: number | null;
};

type WorkoutSegment = {
  order: number;
  label: string;
  durationSeconds: number;
  startOffsetSeconds: number;
  endOffsetSeconds: number;
  targetPercentFtp: number | null;
  zoneId: number | null;
};

type EventSummary = {
  totalSegments: number;
  totalDurationSeconds: number;
  estimatedNormalizedPowerWatts: number | null;
  estimatedAveragePowerWatts: number | null;
  estimatedIntensityFactor: number | null;
  estimatedTrainingStressScore: number | null;
};

type IntervalEvent = {
  id: number;
  calendarEntryId?: string;
  startDateLocal: string;
  name: string | null;
  category: string;
  description: string | null;
  restDay?: boolean;
  restDayReason?: string | null;
  indoor: boolean;
  color: string | null;
  eventDefinition: {
    rawWorkoutDoc: string | null;
    intervals: IntervalDefinition[];
    segments: WorkoutSegment[];
    summary: EventSummary;
  };
  actualWorkout: ActualWorkout | null;
  plannedSource?: 'intervals' | 'predicted';
  syncStatus?: 'unsynced' | 'pending' | 'synced' | 'modified' | 'failed' | null;
  linkedIntervalsEventId?: number | null;
  projectedWorkout?: {
    projectedWorkoutId: string;
    operationKey: string;
    date: string;
    sourceWorkoutId: string;
    restDay?: boolean;
    restDayReason?: string | null;
  } | null;
};

type ActualWorkout = {
  activityId: string;
  activityName: string | null;
  startDateLocal: string;
  powerValues: number[];
  cadenceValues: number[];
  heartRateValues: number[];
  speedValues: number[];
  averagePowerWatts: number | null;
  normalizedPowerWatts: number | null;
  trainingStressScore: number | null;
  intensityFactor: number | null;
  complianceScore: number;
  matchedIntervals: Array<{
    plannedSegmentOrder: number;
    plannedLabel: string;
    plannedDurationSeconds: number;
    targetPercentFtp: number | null;
    zoneId: number | null;
    actualIntervalId: number | null;
    actualStartTimeSeconds: number | null;
    actualEndTimeSeconds: number | null;
    averagePowerWatts: number | null;
    normalizedPowerWatts: number | null;
    averageHeartRateBpm: number | null;
    averageCadenceRpm: number | null;
    averageSpeedMps: number | null;
    complianceScore: number;
  }>;
};

type IntervalActivity = {
  id: string;
  startDateLocal: string;
  startDate: string | null;
  name: string | null;
  description: string | null;
  activityType: string | null;
  source: string | null;
  externalId: string | null;
  deviceName: string | null;
  distanceMeters: number | null;
  movingTimeSeconds: number | null;
  elapsedTimeSeconds: number | null;
  totalElevationGainMeters: number | null;
  averageSpeedMps: number | null;
  averageHeartRateBpm: number | null;
  averageCadenceRpm: number | null;
  trainer: boolean;
  commute: boolean;
  race: boolean;
  hasHeartRate: boolean;
  streamTypes: string[];
  tags: string[];
  metrics: {
    trainingStressScore: number | null;
    normalizedPowerWatts: number | null;
    intensityFactor: number | null;
    efficiencyFactor: number | null;
    variabilityIndex: number | null;
    averagePowerWatts: number | null;
    ftpWatts: number | null;
    totalWorkJoules: number | null;
    calories: number | null;
    trimp: number | null;
    powerLoad: number | null;
    heartRateLoad: number | null;
    paceLoad: number | null;
    strainScore: number | null;
  };
  details: {
    intervals: Array<{
      id: number | null;
      label: string | null;
      intervalType: string | null;
      groupId: string | null;
      startIndex: number | null;
      endIndex: number | null;
      startTimeSeconds: number | null;
      endTimeSeconds: number | null;
      movingTimeSeconds: number | null;
      elapsedTimeSeconds: number | null;
      distanceMeters: number | null;
      averagePowerWatts: number | null;
      normalizedPowerWatts: number | null;
      trainingStressScore: number | null;
      averageHeartRateBpm: number | null;
      averageCadenceRpm: number | null;
      averageSpeedMps: number | null;
      averageStrideMeters: number | null;
      zone: number | null;
    }>;
    intervalGroups: Array<{
      id: string;
      count: number | null;
      startIndex: number | null;
      movingTimeSeconds: number | null;
      elapsedTimeSeconds: number | null;
      distanceMeters: number | null;
      averagePowerWatts: number | null;
      normalizedPowerWatts: number | null;
      trainingStressScore: number | null;
      averageHeartRateBpm: number | null;
      averageCadenceRpm: number | null;
      averageSpeedMps: number | null;
      averageStrideMeters: number | null;
    }>;
    streams: Array<{
      streamType: string;
      name: string | null;
      data: unknown;
      data2: unknown;
      valueTypeIsArray: boolean;
      custom: boolean;
      allNull: boolean;
    }>;
    intervalSummary: string[];
    skylineChart: string[];
    powerZoneTimes: Array<{ zoneId: string; seconds: number }>;
    heartRateZoneTimes: number[];
    paceZoneTimes: number[];
    gapZoneTimes: number[];
  };
  detailsUnavailableReason?: string | null;
};

type WorkoutMessage = {
  id: string;
  role: 'user' | 'coach' | 'system' | 'tool';
  content: string;
  toolCall?: {
    id: string;
    name: string;
    argumentsJson: string;
    argumentsPreview?: string | null;
  } | null;
  questions?: Array<{
    id: string;
    question: string;
    answers: string[];
    freeTextLabel?: string | null;
  }> | null;
  createdAtEpochSeconds: number;
};

type WorkoutSummary = {
  id: string;
  workoutId: string;
  rpe: number | null;
  messages: WorkoutMessage[];
  savedAtEpochSeconds: number | null;
  createdAtEpochSeconds: number;
  updatedAtEpochSeconds: number;
};

type CalendarCoachConversation = {
  conversationId: string;
  surface: 'calendar';
  status: 'active' | 'archived';
  focus: 'overview';
  createdAtEpochSeconds: number;
  updatedAtEpochSeconds: number;
};

type CalendarCoachMessage = {
  id: string;
  role: 'user' | 'coach' | 'system' | 'tool';
  content: string;
  toolCall?: {
    id: string;
    name: string;
    argumentsJson: string;
    argumentsPreview?: string | null;
  } | null;
  createdAtEpochSeconds: number;
};

type ScheduledTask = {
  id: string;
  userId: string;
  taskType: string;
  status: 'queued' | 'running' | 'retry_scheduled' | 'failed' | 'completed' | 'timed_out' | 'cancelled';
  payload: unknown;
  checkpoint: unknown | null;
  retryStrategy:
    | { kind: 'never' }
    | { kind: 'fixed'; maxAttempts: number; delaySeconds: number }
    | { kind: 'exponential'; maxAttempts: number; initialDelaySeconds: number; maxDelaySeconds: number };
  dedupeKey: string;
  errorMessage: string | null;
  attemptCount: number;
  nextAttemptAtEpochSeconds: number;
  claimedBy: string | null;
  leaseExpiresAtEpochSeconds: number | null;
  lastHeartbeatAtEpochSeconds: number | null;
  executionTimeoutSeconds: number;
  timedOutAtEpochSeconds: number | null;
  leaderOnly: boolean;
  createdAtEpochSeconds: number;
  updatedAtEpochSeconds: number;
  startedAtEpochSeconds: number | null;
  finishedAtEpochSeconds: number | null;
};

type Race = {
  raceId: string;
  date: string;
  name: string;
  distanceMeters: number;
  discipline: 'road' | 'mtb' | 'gravel' | 'cyclocross' | 'timetrial';
  priority: 'A' | 'B' | 'C';
  syncStatus: 'pending' | 'synced' | 'failed' | 'pending_delete';
  linkedIntervalsEventId: number | null;
  lastError: string | null;
  result?: 'finished' | 'dnf' | 'dsq';
};

export type PreviewPresetName = 'balanced' | 'mobile-focus' | 'empty';

export type PreviewPreset = {
  name: PreviewPresetName;
  label: string;
  currentUser: CurrentUser;
  settings: UserSettings;
  dashboardByRange: Record<DashboardRange, DashboardResponse>;
  events: IntervalEvent[];
  activities: IntervalActivity[];
  labelsByDate: Record<string, Record<string, unknown>>;
  workoutSummaries: Record<string, WorkoutSummary>;
  completedWorkoutSummaries: Record<string, { workoutId: string; text: string; provider?: string | null; model?: string | null; generatedAtEpochSeconds: number }>;
  calendarCoach: {
    currentConversationId: string | null;
    conversations: Record<string, { conversation: CalendarCoachConversation; messages: CalendarCoachMessage[] }>;
  };
  tasks: ScheduledTask[];
  races: Race[];
  athleteSummary: {
    exists: boolean;
    stale: boolean;
    summaryText: string | null;
    generatedAtEpochSeconds: number | null;
    updatedAtEpochSeconds: number | null;
  };
};

const DAY_MS = 24 * 60 * 60 * 1000;

function startOfDay(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function addDays(date: Date, days: number) {
  return new Date(date.getTime() + days * DAY_MS);
}

function toDateKey(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function toLocalIso(date: Date, hour = 7, minute = 0) {
  const value = new Date(date.getFullYear(), date.getMonth(), date.getDate(), hour, minute, 0);
  const year = value.getFullYear();
  const month = String(value.getMonth() + 1).padStart(2, '0');
  const day = String(value.getDate()).padStart(2, '0');
  const hours = String(value.getHours()).padStart(2, '0');
  const minutes = String(value.getMinutes()).padStart(2, '0');
  const seconds = String(value.getSeconds()).padStart(2, '0');
  return `${year}-${month}-${day}T${hours}:${minutes}:${seconds}`;
}

function unix(date: Date) {
  return Math.floor(date.getTime() / 1000);
}

function makeAvailability(): AvailabilityDay[] {
  return [
    { weekday: 'mon', available: true, maxDurationMinutes: 60 },
    { weekday: 'tue', available: false, maxDurationMinutes: null },
    { weekday: 'wed', available: true, maxDurationMinutes: 90 },
    { weekday: 'thu', available: true, maxDurationMinutes: 120 },
    { weekday: 'fri', available: true, maxDurationMinutes: 60 },
    { weekday: 'sat', available: true, maxDurationMinutes: 180 },
    { weekday: 'sun', available: false, maxDurationMinutes: null },
  ];
}

function makeSettings(now: Date): UserSettings {
  return {
    aiAgents: {
      openaiApiKey: 'sk-preview-redacted',
      openaiApiKeySet: true,
      geminiApiKey: null,
      geminiApiKeySet: false,
      openrouterApiKey: null,
      openrouterApiKeySet: false,
      deepseekApiKey: null,
      deepseekApiKeySet: false,
      zaiApiKey: null,
      zaiApiKeySet: false,
      selectedProvider: 'openai',
      selectedModel: 'gpt-5.4',
    },
    intervals: {
      apiKey: 'intervals-preview-redacted',
      apiKeySet: true,
      athleteId: 'i123456',
      connected: true,
    },
    wahoo: {
      available: true,
      accessToken: null,
      accessTokenSet: false,
      refreshTokenSet: false,
      expiresAtEpochSeconds: null,
      connected: false,
    },
    options: {
      analyzeWithoutHeartRate: true,
    },
    availability: {
      configured: true,
      days: makeAvailability(),
    },
    cycling: {
      fullName: 'Alex Rivier',
      age: 31,
      heightCm: 182,
      weightKg: 74,
      ftpWatts: 286,
      hrMaxBpm: 192,
      vo2Max: 62,
      athletePrompt: 'Strong amateur cyclist preparing for fondos and short stage races.',
      medications: 'Seasonal antihistamine as needed.',
      athleteNotes: 'Responds well to clear structure and short tactical reminders.',
      lastZoneUpdateEpochSeconds: unix(addDays(now, -12)),
    },
  };
}

function createPowerValues(length: number, base: number, variance: number) {
  return Array.from({ length }, (_, index) => Math.max(90, Math.round(base + Math.sin(index / 6) * variance + (index % 9) * 3)));
}

function createHeartRateValues(length: number, base: number) {
  return Array.from({ length }, (_, index) => Math.round(base + Math.sin(index / 10) * 6 + (index % 5)));
}

function createCadenceValues(length: number, base: number) {
  return Array.from({ length }, (_, index) => Math.round(base + Math.sin(index / 7) * 5));
}

function createSpeedValues(length: number, base: number) {
  return Array.from({ length }, (_, index) => Number((base + Math.sin(index / 12) * 0.4).toFixed(2)));
}

function makePlannedEvent(id: number, date: Date, name: string, segments: Array<{ label: string; durationSeconds: number; targetPercentFtp: number | null; zoneId: number | null }>, options?: {
  description?: string;
  syncStatus?: IntervalEvent['syncStatus'];
  linkedIntervalsEventId?: number | null;
  indoor?: boolean;
  actualWorkout?: ActualWorkout | null;
  rawWorkoutDoc?: string;
  restDay?: boolean;
  restDayReason?: string | null;
}) : IntervalEvent {
  let offset = 0;
  const workoutSegments: WorkoutSegment[] = segments.map((segment, index) => {
    const next = {
      order: index,
      label: segment.label,
      durationSeconds: segment.durationSeconds,
      startOffsetSeconds: offset,
      endOffsetSeconds: offset + segment.durationSeconds,
      targetPercentFtp: segment.targetPercentFtp,
      zoneId: segment.zoneId,
    };
    offset += segment.durationSeconds;
    return next;
  });

  return {
    id,
    calendarEntryId: `calendar-entry-${id}`,
    startDateLocal: toLocalIso(date, options?.restDay ? 0 : 7),
    name,
    category: 'WORKOUT',
    description: options?.description ?? 'Structured bike workout',
    restDay: options?.restDay ?? false,
    restDayReason: options?.restDayReason ?? null,
    indoor: options?.indoor ?? false,
    color: '#7dd3fc',
    eventDefinition: {
      rawWorkoutDoc: options?.rawWorkoutDoc ?? `${name}\n${segments.map((segment) => `${segment.label} ${Math.round(segment.durationSeconds / 60)}min`).join('\n')}`,
      intervals: workoutSegments.map((segment) => ({
        definition: segment.label,
        repeatCount: 1,
        durationSeconds: segment.durationSeconds,
        targetPercentFtp: segment.targetPercentFtp,
        zoneId: segment.zoneId,
      })),
      segments: workoutSegments,
      summary: {
        totalSegments: workoutSegments.length,
        totalDurationSeconds: workoutSegments.reduce((sum, segment) => sum + segment.durationSeconds, 0),
        estimatedNormalizedPowerWatts: 244,
        estimatedAveragePowerWatts: 231,
        estimatedIntensityFactor: 0.87,
        estimatedTrainingStressScore: 82,
      },
    },
    actualWorkout: options?.actualWorkout ?? null,
    plannedSource: 'predicted',
    syncStatus: options?.syncStatus ?? 'modified',
    linkedIntervalsEventId: options?.linkedIntervalsEventId ?? null,
    projectedWorkout: {
      projectedWorkoutId: `projected-${id}`,
      operationKey: `training-plan:user-1:w${Math.max(1, Math.floor(id / 10))}:${id}`,
      date: toDateKey(date),
      sourceWorkoutId: `source-${id}`,
      restDay: options?.restDay ?? false,
      restDayReason: options?.restDayReason ?? null,
    },
  };
}

function makeActualWorkout(activityId: string, date: Date, matchedSegments: Array<{ label: string; durationSeconds: number; targetPercentFtp: number | null; zoneId: number | null; avgPower: number }>): ActualWorkout {
  let offset = 0;
  return {
    activityId,
    activityName: 'Threshold Builder',
    startDateLocal: toLocalIso(date, 6, 30),
    powerValues: createPowerValues(120, 242, 34),
    cadenceValues: createCadenceValues(120, 90),
    heartRateValues: createHeartRateValues(120, 156),
    speedValues: createSpeedValues(120, 9.4),
    averagePowerWatts: 229,
    normalizedPowerWatts: 248,
    trainingStressScore: 88,
    intensityFactor: 0.9,
    complianceScore: 0.91,
    matchedIntervals: matchedSegments.map((segment, index) => {
      const currentOffset = offset;
      offset += segment.durationSeconds;
      return {
        plannedSegmentOrder: index,
        plannedLabel: segment.label,
        plannedDurationSeconds: segment.durationSeconds,
        targetPercentFtp: segment.targetPercentFtp,
        zoneId: segment.zoneId,
        actualIntervalId: index + 1,
        actualStartTimeSeconds: currentOffset,
        actualEndTimeSeconds: offset,
        averagePowerWatts: segment.avgPower,
        normalizedPowerWatts: segment.avgPower + 8,
        averageHeartRateBpm: 162 + index * 2,
        averageCadenceRpm: 91,
        averageSpeedMps: 9.6,
        complianceScore: 0.88 + index * 0.02,
      };
    }),
  };
}

function makeActivity(activityId: string, date: Date, name: string, description: string): IntervalActivity {
  return {
    id: activityId,
    startDateLocal: toLocalIso(date, 6, 30),
    startDate: toLocalIso(date, 6, 30),
    name,
    description,
    activityType: 'Ride',
    source: 'intervals',
    externalId: `paired_event_id=${activityId.replace(/\D/g, '')}`,
    deviceName: 'Garmin Edge',
    distanceMeters: 52400,
    movingTimeSeconds: 5400,
    elapsedTimeSeconds: 5580,
    totalElevationGainMeters: 510,
    averageSpeedMps: 9.7,
    averageHeartRateBpm: 156,
    averageCadenceRpm: 88,
    trainer: false,
    commute: false,
    race: false,
    hasHeartRate: true,
    streamTypes: ['watts', 'heartrate', 'cadence'],
    tags: ['preview'],
    metrics: {
      trainingStressScore: 88,
      normalizedPowerWatts: 248,
      intensityFactor: 0.9,
      efficiencyFactor: 1.34,
      variabilityIndex: 1.08,
      averagePowerWatts: 229,
      ftpWatts: 286,
      totalWorkJoules: 1042000,
      calories: 1390,
      trimp: 112,
      powerLoad: 88,
      heartRateLoad: 75,
      paceLoad: null,
      strainScore: 7.2,
    },
    details: {
      intervals: [
        {
          id: 1,
          label: 'Warm up',
          intervalType: 'warmup',
          groupId: 'g1',
          startIndex: 0,
          endIndex: 20,
          startTimeSeconds: 0,
          endTimeSeconds: 900,
          movingTimeSeconds: 900,
          elapsedTimeSeconds: 900,
          distanceMeters: 8400,
          averagePowerWatts: 175,
          normalizedPowerWatts: 182,
          trainingStressScore: 12,
          averageHeartRateBpm: 138,
          averageCadenceRpm: 87,
          averageSpeedMps: 8.9,
          averageStrideMeters: null,
          zone: 2,
        },
        {
          id: 2,
          label: 'Threshold 1',
          intervalType: 'work',
          groupId: 'g2',
          startIndex: 21,
          endIndex: 55,
          startTimeSeconds: 900,
          endTimeSeconds: 2100,
          movingTimeSeconds: 1200,
          elapsedTimeSeconds: 1200,
          distanceMeters: 12200,
          averagePowerWatts: 286,
          normalizedPowerWatts: 294,
          trainingStressScore: 28,
          averageHeartRateBpm: 166,
          averageCadenceRpm: 92,
          averageSpeedMps: 10.4,
          averageStrideMeters: null,
          zone: 4,
        },
      ],
      intervalGroups: [
        {
          id: 'g2',
          count: 2,
          startIndex: 21,
          movingTimeSeconds: 2400,
          elapsedTimeSeconds: 2400,
          distanceMeters: 24400,
          averagePowerWatts: 284,
          normalizedPowerWatts: 292,
          trainingStressScore: 56,
          averageHeartRateBpm: 167,
          averageCadenceRpm: 91,
          averageSpeedMps: 10.2,
          averageStrideMeters: null,
        },
      ],
      streams: [
        {
          streamType: 'watts',
          name: 'Power',
          data: createPowerValues(120, 242, 34),
          data2: null,
          valueTypeIsArray: true,
          custom: false,
          allNull: false,
        },
      ],
      intervalSummary: ['2x20 min threshold with controlled recoveries'],
      skylineChart: ['warmup', 'threshold', 'recovery', 'threshold', 'cooldown'],
      powerZoneTimes: [
        { zoneId: 'z1', seconds: 900 },
        { zoneId: 'z2', seconds: 1200 },
        { zoneId: 'z4', seconds: 2400 },
      ],
      heartRateZoneTimes: [300, 900, 1800, 1800, 600],
      paceZoneTimes: [],
      gapZoneTimes: [],
    },
    detailsUnavailableReason: null,
  };
}

function makeWorkoutSummary(workoutId: string, now: Date, options?: { saved?: boolean; withConversation?: boolean }): WorkoutSummary {
  const createdAtEpochSeconds = unix(addDays(now, -2));
  const baseMessages: WorkoutMessage[] = [
    {
      id: `msg-${workoutId}-1`,
      role: 'system',
      content: 'RPE recorded and summary initialized.',
      createdAtEpochSeconds,
    },
  ];

  if (options?.withConversation) {
    baseMessages.push(
      {
        id: `msg-${workoutId}-2`,
        role: 'user',
        content: 'I felt good on the second threshold block but started the first one too hard.',
        createdAtEpochSeconds: createdAtEpochSeconds + 120,
      },
      {
        id: `msg-${workoutId}-3`,
        role: 'coach',
        content: 'That matches the power drift. Keep the first five minutes 5-8W lower next time and you should finish both blocks cleaner.',
        createdAtEpochSeconds: createdAtEpochSeconds + 240,
      },
    );
  }

  return {
    id: `summary-${workoutId}`,
    workoutId,
    rpe: 7,
    messages: baseMessages,
    savedAtEpochSeconds: options?.saved ? unix(addDays(now, -1)) : null,
    createdAtEpochSeconds,
    updatedAtEpochSeconds: unix(addDays(now, -1)),
  };
}

function makeCalendarConversation(now: Date) {
  const createdAtEpochSeconds = unix(addDays(now, -1));
  const conversationId = 'calendar-conv-1';
  return {
    currentConversationId: conversationId,
    conversations: {
      [conversationId]: {
        conversation: {
          conversationId,
          surface: 'calendar',
          status: 'active',
          focus: 'overview',
          createdAtEpochSeconds,
          updatedAtEpochSeconds: createdAtEpochSeconds + 300,
        },
        messages: [
          {
            id: 'calendar-msg-1',
            role: 'coach',
            content: 'This week is front-loaded. Saturday is still your biggest quality slot, so avoid adding intensity on Friday.',
            createdAtEpochSeconds,
          },
        ],
      },
    },
  } satisfies PreviewPreset['calendarCoach'];
}

function makeTasks(now: Date, dense = false): ScheduledTask[] {
  const base = unix(now);
  const tasks: ScheduledTask[] = [
    {
      id: 'task-training-plan-refresh',
      userId: 'user-1',
      taskType: 'training_plan.refresh',
      status: 'failed',
      payload: { week: toDateKey(now) },
      checkpoint: null,
      retryStrategy: { kind: 'fixed', maxAttempts: 3, delaySeconds: 30 },
      dedupeKey: 'training-plan-refresh:user-1',
      errorMessage: 'Upstream provider timed out while generating plan delta.',
      attemptCount: 2,
      nextAttemptAtEpochSeconds: base + 600,
      claimedBy: null,
      leaseExpiresAtEpochSeconds: null,
      lastHeartbeatAtEpochSeconds: null,
      executionTimeoutSeconds: 180,
      timedOutAtEpochSeconds: null,
      leaderOnly: false,
      createdAtEpochSeconds: base - 1800,
      updatedAtEpochSeconds: base - 300,
      startedAtEpochSeconds: base - 600,
      finishedAtEpochSeconds: base - 300,
    },
    {
      id: 'task-athlete-summary-generate',
      userId: 'user-1',
      taskType: 'athlete_summary.generate',
      status: 'running',
      payload: { athleteId: 'user-1' },
      checkpoint: { step: 'prompting' },
      retryStrategy: { kind: 'fixed', maxAttempts: 3, delaySeconds: 30 },
      dedupeKey: 'athlete-summary:user-1',
      errorMessage: null,
      attemptCount: 1,
      nextAttemptAtEpochSeconds: base,
      claimedBy: 'worker-preview-1',
      leaseExpiresAtEpochSeconds: base + 120,
      lastHeartbeatAtEpochSeconds: base - 5,
      executionTimeoutSeconds: 180,
      timedOutAtEpochSeconds: null,
      leaderOnly: false,
      createdAtEpochSeconds: base - 240,
      updatedAtEpochSeconds: base - 5,
      startedAtEpochSeconds: base - 240,
      finishedAtEpochSeconds: null,
    },
  ];

  if (dense) {
    tasks.push(
      {
        id: 'task-wahoo-sync-preview',
        userId: 'user-1',
        taskType: 'planned_workout.sync_wahoo',
        status: 'queued',
        payload: { day: toDateKey(addDays(now, 2)) },
        checkpoint: null,
        retryStrategy: { kind: 'never' },
        dedupeKey: 'planned-wahoo-sync:user-1',
        errorMessage: null,
        attemptCount: 0,
        nextAttemptAtEpochSeconds: base + 180,
        claimedBy: null,
        leaseExpiresAtEpochSeconds: null,
        lastHeartbeatAtEpochSeconds: null,
        executionTimeoutSeconds: 180,
        timedOutAtEpochSeconds: null,
        leaderOnly: true,
        createdAtEpochSeconds: base - 120,
        updatedAtEpochSeconds: base - 120,
        startedAtEpochSeconds: null,
        finishedAtEpochSeconds: null,
      },
    );
  }

  return tasks;
}

function makeRaces(now: Date): Race[] {
  return [
    {
      raceId: 'race-a',
      date: toDateKey(addDays(now, 21)),
      name: 'Mazovia Gravel 120',
      distanceMeters: 120000,
      discipline: 'gravel',
      priority: 'A',
      syncStatus: 'synced',
      linkedIntervalsEventId: 901,
      lastError: null,
    },
    {
      raceId: 'race-b',
      date: toDateKey(addDays(now, -12)),
      name: 'City TT Series',
      distanceMeters: 24000,
      discipline: 'timetrial',
      priority: 'B',
      syncStatus: 'pending',
      linkedIntervalsEventId: null,
      lastError: null,
      result: 'finished',
    },
  ];
}

function buildDashboard(now: Date, range: DashboardRange): DashboardResponse {
  const pointCount = range === '90d' ? 16 : range === 'season' ? 22 : 28;
  const points: DashboardPoint[] = Array.from({ length: pointCount }, (_, index) => {
    const date = addDays(now, -(pointCount - index - 1) * (range === '90d' ? 5 : range === 'season' ? 8 : 14));
    const ctl = 62 + index * 0.8;
    const atl = 74 + Math.sin(index / 2) * 10;
    return {
      date: toDateKey(date),
      dailyTss: index % 4 === 0 ? 0 : 48 + (index % 6) * 14,
      currentCtl: Number(ctl.toFixed(1)),
      currentAtl: Number(atl.toFixed(1)),
      currentTsb: Number((ctl - atl).toFixed(1)),
    };
  });

  return {
    range,
    windowStart: points[0]?.date ?? toDateKey(addDays(now, -90)),
    windowEnd: points[points.length - 1]?.date ?? toDateKey(now),
    hasTrainingLoad: true,
    summary: {
      currentCtl: 74.2,
      currentAtl: 82.5,
      currentTsb: -8.3,
      ftpWatts: 286,
      averageIf28d: 0.83,
      averageEf28d: 1.31,
      loadDeltaCtl14d: 4.7,
      tsbZone: 'optimal_training',
    },
    points,
  };
}

function makeBalancedPreset(): PreviewPreset {
  const now = startOfDay(new Date());
  const thresholdDate = addDays(now, -1);
  const enduranceDate = addDays(now, -3);
  const futureDate = addDays(now, 2);
  const easyDate = addDays(now, 4);
  const actualWorkout = makeActualWorkout('activity-401', thresholdDate, [
    { label: 'Warm up', durationSeconds: 900, targetPercentFtp: 0.65, zoneId: 2, avgPower: 182 },
    { label: 'Threshold 1', durationSeconds: 1200, targetPercentFtp: 0.98, zoneId: 4, avgPower: 287 },
    { label: 'Recovery', durationSeconds: 300, targetPercentFtp: 0.55, zoneId: 1, avgPower: 160 },
    { label: 'Threshold 2', durationSeconds: 1200, targetPercentFtp: 0.98, zoneId: 4, avgPower: 283 },
    { label: 'Cool down', durationSeconds: 900, targetPercentFtp: 0.55, zoneId: 1, avgPower: 170 },
  ]);
  const thresholdEvent = makePlannedEvent(401, thresholdDate, 'Threshold Builder', [
    { label: 'Warm up', durationSeconds: 900, targetPercentFtp: 0.65, zoneId: 2 },
    { label: 'Threshold 1', durationSeconds: 1200, targetPercentFtp: 0.98, zoneId: 4 },
    { label: 'Recovery', durationSeconds: 300, targetPercentFtp: 0.55, zoneId: 1 },
    { label: 'Threshold 2', durationSeconds: 1200, targetPercentFtp: 0.98, zoneId: 4 },
    { label: 'Cool down', durationSeconds: 900, targetPercentFtp: 0.55, zoneId: 1 },
  ], {
    actualWorkout,
    syncStatus: 'synced',
    linkedIntervalsEventId: 401,
  });
  const enduranceEvent = makePlannedEvent(402, enduranceDate, 'Endurance Ride', [
    { label: 'Endurance', durationSeconds: 5400, targetPercentFtp: 0.72, zoneId: 2 },
  ], {
    syncStatus: 'modified',
  });
  const futureEvent = makePlannedEvent(403, futureDate, 'VO2 Touches', [
    { label: 'Warm up', durationSeconds: 900, targetPercentFtp: 0.62, zoneId: 2 },
    { label: 'VO2 1', durationSeconds: 180, targetPercentFtp: 1.15, zoneId: 5 },
    { label: 'Recovery', durationSeconds: 180, targetPercentFtp: 0.5, zoneId: 1 },
    { label: 'VO2 2', durationSeconds: 180, targetPercentFtp: 1.15, zoneId: 5 },
    { label: 'Cool down', durationSeconds: 600, targetPercentFtp: 0.55, zoneId: 1 },
  ]);
  const easyEvent = makePlannedEvent(404, easyDate, 'Recovery Spin', [
    { label: 'Easy spin', durationSeconds: 2700, targetPercentFtp: 0.55, zoneId: 1 },
  ], {
    syncStatus: 'unsynced',
  });
  const activity = makeActivity('activity-401', thresholdDate, 'Threshold Builder', 'paired_event_id=401');
  const nowForData = addDays(now, 0);

  return {
    name: 'balanced',
    label: 'Balanced populated preview',
    currentUser: {
      id: 'user-1',
      email: 'preview@aiwattcoach.local',
      displayName: 'Preview Athlete',
      avatarUrl: null,
      roles: ['user', 'admin'],
    },
    settings: makeSettings(nowForData),
    dashboardByRange: {
      '90d': buildDashboard(nowForData, '90d'),
      season: buildDashboard(nowForData, 'season'),
      'all-time': buildDashboard(nowForData, 'all-time'),
    },
    events: [thresholdEvent, enduranceEvent, futureEvent, easyEvent],
    activities: [activity],
    labelsByDate: {
      [toDateKey(futureDate)]: {
        raceLabel1: {
          kind: 'race',
          title: 'Mazovia Gravel 120',
          subtitle: 'A priority',
          payload: {
            raceId: 'race-a',
            date: toDateKey(addDays(nowForData, 21)),
            name: 'Mazovia Gravel 120',
            distanceMeters: 120000,
            discipline: 'gravel',
            priority: 'A',
            syncStatus: 'synced',
            linkedIntervalsEventId: 901,
          },
        },
      },
    },
    workoutSummaries: {
      'activity-401': makeWorkoutSummary('activity-401', nowForData, { withConversation: true }),
    },
    completedWorkoutSummaries: {
      'activity-401': {
        workoutId: 'activity-401',
        text: 'Strong threshold session. Second interval stayed remarkably stable after the midpoint and cadence remained controlled throughout.',
        provider: 'openai',
        model: 'gpt-5.4',
        generatedAtEpochSeconds: unix(addDays(nowForData, -1)),
      },
    },
    calendarCoach: makeCalendarConversation(nowForData),
    tasks: makeTasks(nowForData, false),
    races: makeRaces(nowForData),
    athleteSummary: {
      exists: true,
      stale: false,
      summaryText: 'Consistent aerobic base, strong threshold repeatability, and good adherence to structured work. Fatigue trends suggest avoiding stacked intensity before Saturday.',
      generatedAtEpochSeconds: unix(addDays(nowForData, -2)),
      updatedAtEpochSeconds: unix(addDays(nowForData, -2)),
    },
  };
}

function makeMobileFocusPreset(): PreviewPreset {
  const base = makeBalancedPreset();
  const now = startOfDay(new Date());
  const extraEvents = [
    makePlannedEvent(405, addDays(now, -6), 'Tempo Sandwich', [
      { label: 'Warm up', durationSeconds: 900, targetPercentFtp: 0.62, zoneId: 2 },
      { label: 'Tempo', durationSeconds: 1800, targetPercentFtp: 0.84, zoneId: 3 },
      { label: 'Easy', durationSeconds: 600, targetPercentFtp: 0.55, zoneId: 1 },
      { label: 'Tempo', durationSeconds: 1800, targetPercentFtp: 0.84, zoneId: 3 },
    ]),
    makePlannedEvent(406, addDays(now, 1), 'Openers', [
      { label: 'Openers', durationSeconds: 2100, targetPercentFtp: 0.7, zoneId: 2 },
    ], { syncStatus: 'pending' }),
    makePlannedEvent(407, addDays(now, 5), 'Long Endurance', [
      { label: 'Base ride', durationSeconds: 7200, targetPercentFtp: 0.68, zoneId: 2 },
    ], { syncStatus: 'failed' }),
  ];

  return {
    ...base,
    name: 'mobile-focus',
    label: 'Dense mobile review preview',
    events: [...base.events, ...extraEvents],
    tasks: makeTasks(now, true),
    calendarCoach: {
      currentConversationId: 'calendar-conv-mobile',
      conversations: {
        'calendar-conv-mobile': {
          conversation: {
            conversationId: 'calendar-conv-mobile',
            surface: 'calendar',
            status: 'active',
            focus: 'overview',
            createdAtEpochSeconds: unix(addDays(now, -1)),
            updatedAtEpochSeconds: unix(now),
          },
          messages: [
            {
              id: 'calendar-mobile-1',
              role: 'coach',
              content: 'You have three key touchpoints in the next six days. Keep Thursday genuinely easy so Saturday can stay sharp.',
              createdAtEpochSeconds: unix(addDays(now, -1)),
            },
            {
              id: 'calendar-mobile-2',
              role: 'tool',
              content: 'Updated planned workout for Friday.',
              toolCall: {
                id: 'tool-update-1',
                name: 'update_planned_workout',
                argumentsJson: '{"date":"2026-05-29"}',
                argumentsPreview: 'Fri workout shifted by 15 min',
              },
              createdAtEpochSeconds: unix(addDays(now, -1)) + 60,
            },
          ],
        },
      },
    },
  };
}

function makeEmptyPreset(): PreviewPreset {
  const now = startOfDay(new Date());
  const settings = makeSettings(now);
  settings.intervals.connected = false;
  settings.wahoo.available = false;
  settings.aiAgents.openaiApiKeySet = false;
  settings.aiAgents.openaiApiKey = null;
  settings.aiAgents.selectedProvider = null;
  settings.aiAgents.selectedModel = null;
  settings.availability.configured = false;
  settings.availability.days = settings.availability.days.map((day) => ({ ...day, available: false, maxDurationMinutes: null }));

  return {
    name: 'empty',
    label: 'Mostly empty state preview',
    currentUser: {
      id: 'user-1',
      email: 'preview@aiwattcoach.local',
      displayName: 'Preview Athlete',
      avatarUrl: null,
      roles: ['user', 'admin'],
    },
    settings,
    dashboardByRange: {
      '90d': {
        range: '90d',
        windowStart: toDateKey(addDays(now, -90)),
        windowEnd: toDateKey(now),
        hasTrainingLoad: false,
        summary: {
          currentCtl: null,
          currentAtl: null,
          currentTsb: null,
          ftpWatts: null,
          averageIf28d: null,
          averageEf28d: null,
          loadDeltaCtl14d: null,
          tsbZone: 'freshness_peak',
        },
        points: [],
      },
      season: {
        range: 'season',
        windowStart: toDateKey(addDays(now, -180)),
        windowEnd: toDateKey(now),
        hasTrainingLoad: false,
        summary: {
          currentCtl: null,
          currentAtl: null,
          currentTsb: null,
          ftpWatts: null,
          averageIf28d: null,
          averageEf28d: null,
          loadDeltaCtl14d: null,
          tsbZone: 'freshness_peak',
        },
        points: [],
      },
      'all-time': {
        range: 'all-time',
        windowStart: toDateKey(addDays(now, -365)),
        windowEnd: toDateKey(now),
        hasTrainingLoad: false,
        summary: {
          currentCtl: null,
          currentAtl: null,
          currentTsb: null,
          ftpWatts: null,
          averageIf28d: null,
          averageEf28d: null,
          loadDeltaCtl14d: null,
          tsbZone: 'freshness_peak',
        },
        points: [],
      },
    },
    events: [],
    activities: [],
    labelsByDate: {},
    workoutSummaries: {},
    completedWorkoutSummaries: {},
    calendarCoach: {
      currentConversationId: null,
      conversations: {},
    },
    tasks: [],
    races: [],
    athleteSummary: {
      exists: false,
      stale: false,
      summaryText: null,
      generatedAtEpochSeconds: null,
      updatedAtEpochSeconds: null,
    },
  };
}

export function createPreset(name: string | undefined): PreviewPreset {
  switch (name) {
    case 'mobile-focus':
      return makeMobileFocusPreset();
    case 'empty':
      return makeEmptyPreset();
    case 'balanced':
    case undefined:
    case '':
      return makeBalancedPreset();
    default:
      return makeBalancedPreset();
  }
}

export const previewPresetNames: PreviewPresetName[] = ['balanced', 'mobile-focus', 'empty'];
