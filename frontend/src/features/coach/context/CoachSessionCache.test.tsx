import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { CoachSessionCacheProvider, useCoachSessionCache } from './CoachSessionCache';
import type { WorkoutSummary } from '../types';

function buildSummary(overrides: Partial<WorkoutSummary> = {}): WorkoutSummary {
  return {
    id: 'summary-1',
    workoutId: 'activity-101',
    rpe: 6,
    hasCoachMessage: true,
    messages: [
      {
        id: 'message-1',
        role: 'coach',
        content: 'Nice work.',
        createdAtEpochSeconds: 1,
      },
    ],
    savedAtEpochSeconds: null,
    createdAtEpochSeconds: 1,
    updatedAtEpochSeconds: 2,
    ...overrides,
  };
}

function renderCacheHook() {
  return renderHook(() => useCoachSessionCache(), {
    wrapper: ({ children }) => <CoachSessionCacheProvider>{children}</CoachSessionCacheProvider>,
  });
}

describe('CoachSessionCache', () => {
  it('does not let a stale empty metadata response delete a newer cached full summary', () => {
    const { result } = renderCacheHook();

    act(() => {
      result.current.upsertFullSummary(buildSummary({ updatedAtEpochSeconds: 5, rpe: 8 }));
    });

    act(() => {
      result.current.hydrateMetadataSummaries(['activity-101'], []);
    });

    expect(result.current.getSummary('activity-101')).toEqual(
      buildSummary({ updatedAtEpochSeconds: 5, rpe: 8 }),
    );
  });

  it('does not let metadata replacement wipe cached full-summary messages', () => {
    const { result } = renderCacheHook();

    act(() => {
      result.current.upsertFullSummary(buildSummary());
    });

    act(() => {
      result.current.hydrateMetadataSummaries(['activity-101'], [
        buildSummary({
          messages: [],
          updatedAtEpochSeconds: 3,
          savedAtEpochSeconds: 3,
        }),
      ]);
    });

    expect(result.current.getSummary('activity-101')).toEqual(
      buildSummary({
        messages: [
          {
            id: 'message-1',
            role: 'coach',
            content: 'Nice work.',
            createdAtEpochSeconds: 1,
          },
        ],
        updatedAtEpochSeconds: 3,
        savedAtEpochSeconds: 3,
      }),
    );
  });

  it('ignores stale metadata that arrives after a newer cached summary', () => {
    const { result } = renderCacheHook();

    act(() => {
      result.current.upsertFullSummary(buildSummary({ updatedAtEpochSeconds: 5, rpe: 8 }));
    });

    act(() => {
      result.current.hydrateMetadataSummaries(['activity-101'], [
        buildSummary({
          messages: [],
          updatedAtEpochSeconds: 4,
          rpe: 6,
        }),
      ]);
    });

    expect(result.current.getSummary('activity-101')).toEqual(
      buildSummary({ updatedAtEpochSeconds: 5, rpe: 8 }),
    );
  });
});
