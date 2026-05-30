import { act, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { getWorkoutSummary, saveWorkoutSummary } from '../api/workoutSummary';
import { useCoachChat } from './useCoachChat';
import {
  installFakeWebSocket,
  resetCoachChatTestEnvironment,
  summaryFixture,
} from './useCoachChat.testUtils';
import { renderCoachHook } from './testRender';

vi.mock('../api/workoutSummary', () => ({
  createWorkoutSummary: vi.fn(),
  getWorkoutSummary: vi.fn(),
  reopenWorkoutSummary: vi.fn(),
  saveWorkoutSummary: vi.fn(),
  sendWorkoutSummaryMessage: vi.fn(),
  updateWorkoutSummaryRpe: vi.fn(),
}));

const originalLocation = window.location;
const originalWebSocket = global.WebSocket;

afterEach(() => {
  resetCoachChatTestEnvironment(originalWebSocket, originalLocation);
});

describe('useCoachChat stale selection guards', () => {
  function renderCoachChatHook<TProps>(callback: (props: TProps) => ReturnType<typeof useCoachChat>, options: {
    initialProps: TProps;
  }) {
    return renderCoachHook(callback, options);
  }

  it('ignores stale load responses after switching workouts', async () => {
    let resolveFirstSummary: ((value: typeof summaryFixture) => void) | undefined;
    let resolveSecondSummary: ((value: typeof summaryFixture) => void) | undefined;

    installFakeWebSocket();
    vi.mocked(getWorkoutSummary)
      .mockImplementationOnce(() => new Promise((resolve) => {
        resolveFirstSummary = resolve;
      }))
      .mockImplementationOnce(() => new Promise((resolve) => {
        resolveSecondSummary = resolve;
      }));

    const { result, rerender } = renderCoachChatHook(
      ({ workoutId }) => useCoachChat({ apiBaseUrl: '', workoutId }),
      { initialProps: { workoutId: '101' } },
    );

    rerender({ workoutId: '202' });

    act(() => {
      resolveSecondSummary?.({ ...summaryFixture, workoutId: '202', id: 'summary-202' });
    });

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('202');
    });

    act(() => {
      resolveFirstSummary?.({ ...summaryFixture, workoutId: '101', id: 'summary-101' });
    });

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('202');
    });
  });

  it('ignores stale save results after switching workouts', async () => {
    let resolveSave: ((value: Awaited<ReturnType<typeof saveWorkoutSummary>>) => void) | undefined;

    installFakeWebSocket();
    vi.mocked(getWorkoutSummary)
      .mockResolvedValueOnce(summaryFixture)
      .mockResolvedValueOnce({ ...summaryFixture, workoutId: '202', id: 'summary-202' });
    vi.mocked(saveWorkoutSummary).mockImplementationOnce(() => new Promise((resolve) => {
      resolveSave = resolve;
    }));

    const { result, rerender } = renderCoachChatHook(
      ({ workoutId }) => useCoachChat({ apiBaseUrl: '', workoutId }),
      { initialProps: { workoutId: '101' } },
    );

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('101');
    });

    const savePromise = result.current.saveSummary();
    rerender({ workoutId: '202' });

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('202');
    });

    act(() => {
      resolveSave?.({
        summary: { ...summaryFixture, workoutId: '101', savedAtEpochSeconds: 3 },
        workflow: {
          recapStatus: 'generated',
          planStatus: 'generated',
          messages: ['Workout recap generated.', '14-day schedule generated.'],
        },
      });
    });

    await expect(savePromise).resolves.toBeNull();
    expect(result.current.summary?.workoutId).toBe('202');
    expect(result.current.isSaving).toBe(false);
  });
});
