import { act, waitFor } from '@testing-library/react';
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

describe('useCoachChat save workflow', () => {
  function renderCoachChatHook() {
    return renderCoachHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));
  }

  it('saves draft rpe to the backend', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);
    vi.mocked(updateWorkoutSummaryRpe).mockResolvedValue({ ...summaryFixture, rpe: 9 });
    vi.mocked(saveWorkoutSummary).mockResolvedValue({
      summary: { ...summaryFixture, rpe: 9, savedAtEpochSeconds: 3 },
      workflow: {
        recapStatus: 'generated',
        planStatus: 'skipped',
        messages: ['Workout recap generated.', '14-day schedule skipped because this is not the latest completed activity.'],
      },
    });

    const { result } = renderCoachChatHook();

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
    expect(result.current.messages.at(-2)?.content).toBe('Workout recap generated.');
    expect(result.current.messages.at(-1)?.role).toBe('system');
    expect(result.current.messages.at(-1)?.content).toBe('14-day schedule skipped because this is not the latest completed activity.');
  });

  it('shows saving summary progress for the whole save workflow request', async () => {
    installFakeWebSocket();

    let resolveSave: ((value: Awaited<ReturnType<typeof saveWorkoutSummary>>) => void) | undefined;

    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);
    vi.mocked(saveWorkoutSummary).mockImplementationOnce(() => new Promise((resolve) => {
      resolveSave = resolve;
    }));

    const { result } = renderCoachChatHook();

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

    const { result } = renderCoachChatHook();

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

    const { result } = renderCoachChatHook();

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

    const { result } = renderCoachChatHook();

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

    const { result } = renderCoachChatHook();

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

  it('does not treat a system message as completed conversation', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderCoachChatHook();

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
