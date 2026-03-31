import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import {
  createWorkoutSummary,
  getWorkoutSummary,
  updateWorkoutSummaryRpe,
} from '../api/workoutSummary';
import { useCoachChat } from './useCoachChat';

vi.mock('../api/workoutSummary', () => ({
  createWorkoutSummary: vi.fn(),
  getWorkoutSummary: vi.fn(),
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
  eventId: '101',
  rpe: 7,
  messages: [],
  createdAtEpochSeconds: 1,
  updatedAtEpochSeconds: 2,
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

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', eventId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.eventId).toBe('101');
      expect(result.current.isConnected).toBe(true);
    });

    expect(FakeWebSocket.instances[0]?.url).toContain('/api/workout-summaries/101/ws');
  });

  it('creates a summary on first send when one does not exist', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary).mockRejectedValue(new HttpError(404, 'not found'));
    vi.mocked(createWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', eventId: '101' }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      await result.current.sendMessage('Legs felt strong');
    });

    expect(createWorkoutSummary).toHaveBeenCalledWith('', '101');
    expect(FakeWebSocket.instances[0]?.send).toHaveBeenCalledWith(
      JSON.stringify({ type: 'send_message', content: 'Legs felt strong' }),
    );
  });

  it('saves draft rpe to the backend', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);
    vi.mocked(updateWorkoutSummaryRpe).mockResolvedValue({ ...summaryFixture, rpe: 9 });

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', eventId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.eventId).toBe('101');
    });

    act(() => {
      result.current.setDraftRpe(9);
    });

    await act(async () => {
      await result.current.saveSummary();
    });

    expect(updateWorkoutSummaryRpe).toHaveBeenCalledWith('', '101', 9);
  });

  it('redirects to the landing page on auth failure', async () => {
    vi.mocked(getWorkoutSummary).mockRejectedValue(new AuthenticationError());
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, href: '/ai-coach' },
    });

    renderHook(() => useCoachChat({ apiBaseUrl: '', eventId: '101' }));

    await waitFor(() => {
      expect(window.location.href).toBe('/');
    });
  });
});
