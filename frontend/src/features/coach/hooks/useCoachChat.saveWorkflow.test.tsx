import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { HttpError } from '../../../lib/httpClient';
import {
  getWorkoutSummary,
  reopenWorkoutSummary,
  saveWorkoutSummary,
  updateWorkoutSummaryRpe,
} from '../api/workoutSummary';
import { useCoachChat } from './useCoachChat';
import {
  FakeWebSocket,
  installFakeWebSocket,
  resetCoachChatTestEnvironment,
  summaryFixture,
} from './useCoachChat.testUtils';

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

describe('useCoachChat save workflow', () => {
  it('saves draft rpe to the backend', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);
    vi.mocked(updateWorkoutSummaryRpe).mockResolvedValue({ ...summaryFixture, rpe: 9 });
    vi.mocked(saveWorkoutSummary).mockResolvedValue({
      summary: { ...summaryFixture, rpe: 9, savedAtEpochSeconds: 3 },
      workflow: {
        recapStatus: 'skipped',
        planStatus: 'skipped',
        messages: ['Workout recap skipped.', '14-day schedule skipped.'],
      },
    });

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('101');
    });

    act(() => {
      result.current.setDraftRpe(9);
    });

    await act(async () => {
      await result.current.saveSummary();
    });

    expect(updateWorkoutSummaryRpe).toHaveBeenCalledWith('', '101', 9);
    expect(saveWorkoutSummary).toHaveBeenCalledWith('', '101');
    expect(result.current.isSaved).toBe(true);
    expect(result.current.messages.at(-2)?.role).toBe('system');
    expect(result.current.messages.at(-2)?.content).toBe('Workout recap skipped.');
    expect(result.current.messages.at(-1)?.role).toBe('system');
    expect(result.current.messages.at(-1)?.content).toBe('14-day schedule skipped.');
  });

  it('shows saving summary progress for the whole save workflow request', async () => {
    installFakeWebSocket();

    let resolveSave: ((value: Awaited<ReturnType<typeof saveWorkoutSummary>>) => void) | undefined;

    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);
    vi.mocked(saveWorkoutSummary).mockImplementationOnce(() => new Promise((resolve) => {
      resolveSave = resolve;
    }));

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('101');
    });

    let savePromise: Promise<Awaited<ReturnType<typeof result.current.saveSummary>>> | undefined;

    await act(async () => {
      savePromise = result.current.saveSummary();
    });

    expect(result.current.isSaving).toBe(true);
    expect(result.current.progressState).toBe('saving-summary');

    act(() => {
      resolveSave?.({
        summary: { ...summaryFixture, savedAtEpochSeconds: 3 },
        workflow: {
          recapStatus: 'generated',
          planStatus: 'generated',
          messages: ['Workout recap generated.', '14-day schedule generated.'],
        },
      });
    });

    await act(async () => {
      await savePromise;
    });

    expect(result.current.isSaving).toBe(false);
    expect(result.current.progressState).toBe('idle');
  });

  it('resets saving progress when save workflow fails', async () => {
    installFakeWebSocket();
    let rejectSave: ((reason?: unknown) => void) | undefined;

    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);
    vi.mocked(saveWorkoutSummary).mockImplementationOnce(() => new Promise((_, reject) => {
      rejectSave = reject;
    }));

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('101');
    });

    let savePromise: Promise<Awaited<ReturnType<typeof result.current.saveSummary>>> | undefined;

    await act(async () => {
      savePromise = result.current.saveSummary();
    });

    expect(result.current.isSaving).toBe(true);
    expect(result.current.progressState).toBe('saving-summary');

    act(() => {
      rejectSave?.(new HttpError(500, 'save failed'));
    });

    await act(async () => {
      await savePromise;
    });

    expect(result.current.isSaving).toBe(false);
    expect(result.current.progressState).toBe('idle');
  });

  it('reopens a saved summary for editing', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue({ ...summaryFixture, savedAtEpochSeconds: 3 });
    vi.mocked(reopenWorkoutSummary).mockResolvedValue({
      ...summaryFixture,
      savedAtEpochSeconds: null,
    });

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isSaved).toBe(true);
    });

    await act(async () => {
      await result.current.reopenSummary();
    });

    expect(reopenWorkoutSummary).toHaveBeenCalledWith('', '101');
    expect(result.current.isSaved).toBe(false);
  });

  it('appends failed workflow messages after save', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);
    vi.mocked(saveWorkoutSummary).mockResolvedValue({
      summary: { ...summaryFixture, savedAtEpochSeconds: 3 },
      workflow: {
        recapStatus: 'generated',
        planStatus: 'failed',
        messages: ['Workout recap generated.', '14-day schedule failed.'],
      },
    });

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('101');
    });

    await act(async () => {
      await result.current.saveSummary();
    });

    expect(result.current.messages.at(-2)?.content).toBe('Workout recap generated.');
    expect(result.current.messages.at(-1)?.content).toBe('14-day schedule failed.');
  });

  it('accepts websocket save workflow completion messages', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    act(() => {
      const socket = (global.WebSocket as unknown as typeof FakeWebSocket).instances?.[0] ?? undefined;
      socket?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'save_workflow_complete',
            workflow: {
              recapStatus: 'generated',
              planStatus: 'processing',
              messages: [
                'Workout recap generated.',
                '14-day schedule is being generated in the background.',
              ],
            },
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.messages.at(-2)?.content).toBe('Workout recap generated.');
      expect(result.current.messages.at(-1)?.content).toBe(
        '14-day schedule is being generated in the background.',
      );
    });

    expect(result.current.error).toBeNull();
  });

  it('appends plan quality attempt and finished lines from save workflow messages', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);
    vi.mocked(saveWorkoutSummary).mockResolvedValue({
      summary: { ...summaryFixture, savedAtEpochSeconds: 3 },
      workflow: {
        recapStatus: 'generated',
        planStatus: 'generated',
        messages: [
          'Workout recap generated.',
          '14-day schedule generated.',
          'Plan quality attempt 1/5: 5/10. Too much tempo.',
          'Plan quality attempt 2/5: 8/10. Polarized and race-aware.',
          'Plan quality finished: shipped 8/10 (accepted).',
        ],
      },
    });

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('101');
    });

    await act(async () => {
      await result.current.saveSummary();
    });

    expect(result.current.messages.map((message) => message.content)).toEqual(
      expect.arrayContaining([
        'Plan quality attempt 1/5: 5/10. Too much tempo.',
        'Plan quality attempt 2/5: 8/10. Polarized and race-aware.',
        'Plan quality finished: shipped 8/10 (accepted).',
      ]),
    );
  });

  it('surfaces live plan quality progress system messages over websocket', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    act(() => {
      const socket =
        (global.WebSocket as unknown as typeof FakeWebSocket).instances?.[0] ?? undefined;
      socket?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'system_message',
            content: 'Plan quality attempt 1/5: 5/10. Too much tempo.',
          }),
        }),
      );
      socket?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'system_message',
            content: 'Plan quality finished: shipped 8/10 (accepted).',
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.messages.map((message) => message.content)).toEqual([
        'Plan quality attempt 1/5: 5/10. Too much tempo.',
        'Plan quality finished: shipped 8/10 (accepted).',
      ]);
    });
  });

  it('does not treat a system message as completed conversation', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('101');
      expect(result.current.isConnected).toBe(true);
    });

    const socket = global.WebSocket as unknown as typeof FakeWebSocket;
    act(() => {
      socket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'system_message',
            content: 'First the summary is being generated - wait a moment',
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.messages).toHaveLength(1);
    });

    expect(result.current.hasConversation).toBe(false);
  });
});
