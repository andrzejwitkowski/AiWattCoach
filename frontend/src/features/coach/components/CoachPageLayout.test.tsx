import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import * as calendarHooks from '../../calendar/hooks/useCalendarData';
import * as useWorkoutListModule from '../hooks/useWorkoutList';
import * as useCoachChatModule from '../hooks/useCoachChat';
import type { CoachWorkoutListItem, WorkoutSummary } from '../types';
import { CoachPageLayout } from './CoachPageLayout';

window.HTMLElement.prototype.scrollIntoView = vi.fn();

vi.mock('../../intervals/context', () => ({
  useCompletedWorkouts: vi.fn(),
  CompletedWorkoutsProvider: ({ children }: { children: ReactNode }) => children,
}));

vi.mock('../../calendar/hooks/useCalendarData', () => ({
  invalidateCalendarCache: vi.fn(),
}));

vi.mock('../hooks/useWorkoutList', () => ({ useWorkoutList: vi.fn() }));

vi.mock('../hooks/useCoachChat', () => ({
  useCoachChat: vi.fn(),
  isAvailabilityRequiredChatError: vi.fn().mockReturnValue(false),
}));

vi.mock('../../settings/context/SettingsContext', () => ({ useSettings: vi.fn() }));

import { useCompletedWorkouts } from '../../intervals/context';
import { useSettings } from '../../settings/context/SettingsContext';

const originalLocation = window.location;

const defaultWorkoutItem = (o?: Partial<CoachWorkoutListItem>): CoachWorkoutListItem => ({
  id: 'workout-1',
  source: 'activity',
  startDateLocal: '2025-03-10',
  event: null,
  activity: { id: 'workout-1', name: 'Test Workout', startDateLocal: '2025-03-10', activityType: 'Ride', durationSeconds: 3600 } as never,
  summary: null,
  hasSummary: false,
  hasConversation: false,
  ...o,
});

const defaultSummary = (o?: Partial<WorkoutSummary>): WorkoutSummary => ({
  id: 'summary-1',
  workoutId: 'workout-1',
  rpe: 6,
  messages: [],
  savedAtEpochSeconds: null,
  createdAtEpochSeconds: 1710000000,
  updatedAtEpochSeconds: 1710000000,
  ...o,
});

const availability = [
  { weekday: 'mon', available: true, maxDurationMinutes: 60 },
  { weekday: 'tue', available: true, maxDurationMinutes: 60 },
  { weekday: 'wed', available: true, maxDurationMinutes: 60 },
  { weekday: 'thu', available: true, maxDurationMinutes: 60 },
  { weekday: 'fri', available: true, maxDurationMinutes: 60 },
  { weekday: 'sat', available: false, maxDurationMinutes: null },
  { weekday: 'sun', available: false, maxDurationMinutes: null },
];

function setupMocks(opts?: { planStatus?: 'generated' | 'skipped' | 'failed' | 'unchanged'; saveSummaryResult?: unknown }) {
  const invalidateCalendarCache = vi.mocked(calendarHooks.invalidateCalendarCache);
  const invalidateAll = vi.fn();
  const refresh = vi.fn().mockResolvedValue(undefined);
  const replaceSummary = vi.fn();

  vi.mocked(useCompletedWorkouts).mockReturnValue({
    getActivitiesForRange: vi.fn().mockResolvedValue([]),
    invalidateRange: vi.fn(),
    invalidateAll,
    isLoading: false,
    error: null,
  } as never);

  vi.mocked(useSettings).mockReturnValue({
    settings: { availability },
    isLoading: false,
    error: null,
    saveSettings: vi.fn(),
  } as never);

  vi.mocked(useWorkoutListModule.useWorkoutList).mockReturnValue({
    items: [defaultWorkoutItem()],
    state: 'ready',
    error: null,
    weekLabel: 'Mar 10 - Mar 16',
    canGoToNewerWeek: false,
    goToOlderWeek: vi.fn(),
    goToNewerWeek: vi.fn(),
    refresh,
    replaceSummary,
  } as never);

  vi.mocked(useCoachChatModule.useCoachChat).mockReturnValue({
    summary: null,
    messages: [{ id: 'msg-1', role: 'user', content: 'Test message', createdAtEpochSeconds: 1710000000 }],
    draftRpe: 6,
    isLoading: false,
    isSaving: false,
    isConnected: true,
    isCoachTyping: false,
    progressState: 'idle',
    error: null,
    hasConversation: true,
    isSaved: false,
    setDraftRpe: vi.fn(),
    sendMessage: vi.fn().mockResolvedValue(true),
    saveSummary: opts?.saveSummaryResult !== undefined
      ? vi.fn().mockResolvedValue(opts.saveSummaryResult)
      : vi.fn().mockResolvedValue({
          summary: defaultSummary({ savedAtEpochSeconds: 1710000100 }),
          workflow: { recapStatus: 'generated', planStatus: opts?.planStatus ?? 'generated', messages: [] },
        }),
    reopenSummary: vi.fn().mockResolvedValue(null),
  } as never);

  return { invalidateCalendarCache, invalidateAll, refresh, replaceSummary };
}

beforeEach(() => {
  vi.resetModules();
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  Object.defineProperty(window, 'location', { configurable: true, value: originalLocation });
});

describe('CoachPageLayout', () => {
  it('invalidates caches after save with generated plan', async () => {
    const { invalidateCalendarCache, invalidateAll } = setupMocks({ planStatus: 'generated' });

    render(<CoachPageLayout apiBaseUrl="http://localhost:3000" />);
    fireEvent.click(screen.getByRole('button', { name: /save as workout summary/i }));

    await waitFor(() => expect(invalidateCalendarCache).toHaveBeenCalledTimes(1));
    expect(invalidateAll).toHaveBeenCalledTimes(1);
  });

  it.each([
    ['skipped'],
    ['failed'],
    ['unchanged'],
  ])('does not invalidate caches when plan status is %s', async (planStatus) => {
    const { invalidateCalendarCache, invalidateAll, refresh } = setupMocks({ planStatus: planStatus as never });

    render(<CoachPageLayout apiBaseUrl="http://localhost:3000" />);
    fireEvent.click(screen.getByRole('button', { name: /save as workout summary/i }));

    await waitFor(() => expect(refresh).toHaveBeenCalled());
    expect(invalidateCalendarCache).not.toHaveBeenCalled();
    expect(invalidateAll).not.toHaveBeenCalled();
  });

  it('does not invalidate caches when save returns null', async () => {
    const { invalidateCalendarCache, invalidateAll } = setupMocks({ saveSummaryResult: null });

    render(<CoachPageLayout apiBaseUrl="http://localhost:3000" />);
    fireEvent.click(screen.getByRole('button', { name: /save as workout summary/i }));

    await waitFor(() => expect(invalidateCalendarCache).not.toHaveBeenCalled());
    expect(invalidateAll).not.toHaveBeenCalled();
  });
});
