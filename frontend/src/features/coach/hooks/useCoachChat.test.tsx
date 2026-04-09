import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import {
  createWorkoutSummary,
  getWorkoutSummary,
  reopenWorkoutSummary,
  saveWorkoutSummary,
  updateWorkoutSummaryRpe,
} from '../api/workoutSummary';
import {
  AVAILABILITY_REQUIRED_CHAT_ERROR,
  buildWorkoutSummaryWebSocketUrl,
  isAvailabilityRequiredChatError,
  useCoachChat,
} from './useCoachChat';

vi.mock('../api/workoutSummary', () => ({
  createWorkoutSummary: vi.fn(),
  getWorkoutSummary: vi.fn(),
  reopenWorkoutSummary: vi.fn(),
  saveWorkoutSummary: vi.fn(),
  updateWorkoutSummaryRpe: vi.fn(),
}));

class FakeWebSocket {
  static instances: FakeWebSocket[] = [];
  static OPEN = 1;
  static CLOSED = 3;

  public readyState = 1;
  private listeners = new Map<string, Set<(event?: MessageEvent) => void>>();

  constructor(public readonly url: string) {
    FakeWebSocket.instances.push(this);
    queueMicrotask(() => {
      this.emit('open');
    });
  }

  addEventListener(type: string, listener: (event?: MessageEvent) => void) {
    const current = this.listeners.get(type) ?? new Set();
    current.add(listener);
    this.listeners.set(type, current);
  }

  close() {
    this.readyState = 3;
    this.emit('close');
  }

  send = vi.fn();

  emit(type: string, event?: MessageEvent) {
    this.listeners.get(type)?.forEach((listener) => {
      listener(event);
    });
  }
}

const originalLocation = window.location;
const originalWebSocket = global.WebSocket;

const summaryFixture = {
  id: 'summary-1',
  workoutId: '101',
  rpe: 7,
  messages: [],
  createdAtEpochSeconds: 1,
  updatedAtEpochSeconds: 2,
  savedAtEpochSeconds: null,
};

afterEach(() => {
  vi.clearAllMocks();
  FakeWebSocket.instances = [];
  global.WebSocket = originalWebSocket;
  Object.defineProperty(window, 'location', {
    configurable: true,
    value: originalLocation,
  });
});

describe('useCoachChat', () => {
  it('loads existing summary and connects websocket', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('101');
      expect(result.current.isConnected).toBe(true);
    });

    expect(FakeWebSocket.instances[0]?.url).toContain('/api/workout-summaries/101/ws');
  });

  it('creates a summary on first send when one does not exist', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary).mockRejectedValue(new HttpError(404, 'not found'));
    vi.mocked(createWorkoutSummary).mockResolvedValue({ ...summaryFixture, rpe: 5 });

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    act(() => {
      result.current.setDraftRpe(5);
    });

    await act(async () => {
      await result.current.sendMessage('Legs felt strong');
    });

    expect(createWorkoutSummary).toHaveBeenCalledWith('', '101');
    expect(FakeWebSocket.instances[0]?.send).toHaveBeenCalledWith(
      JSON.stringify({ type: 'send_message', content: 'Legs felt strong' }),
    );
  });

  it('loads existing summary after create conflict', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary)
      .mockRejectedValueOnce(new HttpError(404, 'not found'))
      .mockResolvedValueOnce({ ...summaryFixture, rpe: 5 });
    vi.mocked(createWorkoutSummary).mockRejectedValue(new HttpError(409, 'conflict'));

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    act(() => {
      result.current.setDraftRpe(5);
    });

    await act(async () => {
      await result.current.sendMessage('Legs felt strong');
    });

    expect(createWorkoutSummary).toHaveBeenCalledWith('', '101');
    expect(getWorkoutSummary).toHaveBeenCalledTimes(2);
    expect(result.current.summary?.rpe).toBe(5);
  });

  it('does not create chat session before rpe is chosen', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary).mockRejectedValue(new HttpError(404, 'not found'));

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      await result.current.sendMessage('Legs felt strong');
    });

    expect(createWorkoutSummary).not.toHaveBeenCalled();
    expect(result.current.error).toBeNull();
  });

  it('shows backend availability errors without appending a temporary user message', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    await act(async () => {
      await result.current.sendMessage('Need feedback');
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'error',
            error: AVAILABILITY_REQUIRED_CHAT_ERROR,
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.error).toBe(AVAILABILITY_REQUIRED_CHAT_ERROR);
    });

    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0]?.content).toBe('Need feedback');
  });

  it('recognizes the backend availability error sentinel', () => {
    expect(isAvailabilityRequiredChatError(AVAILABILITY_REQUIRED_CHAT_ERROR)).toBe(true);
    expect(isAvailabilityRequiredChatError('other error')).toBe(false);
    expect(isAvailabilityRequiredChatError(null)).toBe(false);
  });

  it('persists draft rpe before first chat message', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary).mockRejectedValue(new HttpError(404, 'not found'));
    vi.mocked(createWorkoutSummary).mockResolvedValue(summaryFixture);
    vi.mocked(updateWorkoutSummaryRpe).mockResolvedValue({ ...summaryFixture, rpe: 8 });

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    act(() => {
      result.current.setDraftRpe(8);
    });

    await act(async () => {
      await result.current.sendMessage('Legs felt strong');
    });

    expect(updateWorkoutSummaryRpe).toHaveBeenCalledWith('', '101', 8);
  });

  it('saves draft rpe to the backend', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
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
    expect(result.current.messages.at(-2)?.content).toBe('Workout recap generated.');
    expect(result.current.messages.at(-1)?.role).toBe('system');
    expect(result.current.messages.at(-1)?.content).toBe('14-day schedule skipped because this is not the latest completed activity.');
  });

  it('reopens a saved summary for editing', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary).mockResolvedValue({ ...summaryFixture, savedAtEpochSeconds: 3 });
    vi.mocked(reopenWorkoutSummary).mockResolvedValue({
      summary: summaryFixture,
      workflow: { recapStatus: 'unchanged', planStatus: 'unchanged', messages: [] },
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
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
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

  it('does not treat a system message as completed conversation', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('101');
      expect(result.current.isConnected).toBe(true);
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
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

  it('redirects to the landing page on auth failure', async () => {
    vi.mocked(getWorkoutSummary).mockRejectedValue(new AuthenticationError());
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, href: '/ai-coach' },
    });

    renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(window.location.href).toBe('/');
    });
  });

  it('ignores stale load responses after switching workouts', async () => {
    let resolveFirstSummary: ((value: typeof summaryFixture) => void) | undefined;
    let resolveSecondSummary: ((value: typeof summaryFixture) => void) | undefined;

    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary)
      .mockImplementationOnce(() => new Promise((resolve) => {
        resolveFirstSummary = resolve;
      }))
      .mockImplementationOnce(() => new Promise((resolve) => {
        resolveSecondSummary = resolve;
      }));

    const { result, rerender } = renderHook(
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

    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary)
      .mockResolvedValueOnce(summaryFixture)
      .mockResolvedValueOnce({ ...summaryFixture, workoutId: '202', id: 'summary-202' });
    vi.mocked(saveWorkoutSummary).mockImplementationOnce(() => new Promise((resolve) => {
      resolveSave = resolve;
    }));

    const { result, rerender } = renderHook(
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
        workflow: { recapStatus: 'generated', planStatus: 'generated', messages: ['Workout recap generated.', '14-day schedule generated.'] },
      });
    });

    await expect(savePromise).resolves.toBeNull();
    expect(result.current.summary?.workoutId).toBe('202');
    expect(result.current.isSaving).toBe(false);
  });

  it('preserves app path prefixes in websocket urls', () => {
    expect(buildWorkoutSummaryWebSocketUrl('https://example.com/myapp', '101')).toBe(
      'wss://example.com/myapp/api/workout-summaries/101/ws',
    );
  });
});
