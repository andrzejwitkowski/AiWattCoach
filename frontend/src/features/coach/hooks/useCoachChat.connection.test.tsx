import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AuthenticationError } from '../../../lib/httpClient';
import { getWorkoutSummary } from '../api/workoutSummary';
import {
  availabilityRequiredChatError,
  buildWorkoutSummaryWebSocketUrl,
  isAvailabilityRequiredChatError,
  useCoachChat,
} from './useCoachChat';
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

describe('useCoachChat connection', () => {
  it('loads existing summary and connects websocket', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.summary?.workoutId).toBe('101');
      expect(result.current.isConnected).toBe(true);
    });

    expect(FakeWebSocket.instances[0]?.url).toContain('/api/workout-summaries/101/ws');
  });

  it('does not reconnect when rerender receives an equivalent alias range object', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { rerender, result } = renderHook(
      ({ aliasRange }) => useCoachChat({ apiBaseUrl: '', workoutId: '101', aliasRange }),
      { initialProps: { aliasRange: { oldest: '2026-05-25', newest: '2026-05-31' } } },
    );

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    rerender({ aliasRange: { oldest: '2026-05-25', newest: '2026-05-31' } });

    expect(FakeWebSocket.instances).toHaveLength(1);
    expect(getWorkoutSummary).toHaveBeenCalledTimes(1);
  });

  it('does not reconnect when parent cache updates with an empty summary', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { rerender, result } = renderHook(
      ({ cachedSummary }) => useCoachChat({ apiBaseUrl: '', workoutId: '101', cachedSummary }),
      { initialProps: { cachedSummary: null as typeof summaryFixture | null } },
    );

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    rerender({ cachedSummary: { ...summaryFixture } });

    expect(FakeWebSocket.instances).toHaveLength(1);
    expect(getWorkoutSummary).toHaveBeenCalledTimes(1);
  });

  it('does not reconnect when parent cache hydrates with messages for the same workout', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const hydratedSummary = {
      ...summaryFixture,
      messages: [
        {
          id: 'message-1',
          role: 'coach' as const,
          content: 'Keep it easy today.',
          createdAtEpochSeconds: 3,
        },
      ],
    };

    const { rerender, result } = renderHook(
      ({ cachedSummary }) => useCoachChat({ apiBaseUrl: '', workoutId: '101', cachedSummary }),
      { initialProps: { cachedSummary: null as typeof hydratedSummary | null } },
    );

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    rerender({ cachedSummary: hydratedSummary });

    expect(FakeWebSocket.instances).toHaveLength(1);
    expect(getWorkoutSummary).toHaveBeenCalledTimes(1);
  });

  it('keeps a newer websocket coach reply when parent cache sends an older tool-only summary', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const toolOnlySummary = {
      ...summaryFixture,
      updatedAtEpochSeconds: 3,
      messages: [
        {
          id: 'message-user-1',
          role: 'user' as const,
          content: 'Need feedback',
          createdAtEpochSeconds: 2,
        },
        {
          id: 'tool-1',
          role: 'tool' as const,
          content: 'Tool call: get_selected_workout',
          toolCall: {
            id: 'tool-1',
            name: 'get_selected_workout',
            argumentsJson: '{"date":"2026-05-29"}',
            argumentsPreview: 'date 2026-05-29',
          },
          createdAtEpochSeconds: 3,
        },
      ],
    };
    const completeSummary = {
      ...summaryFixture,
      updatedAtEpochSeconds: 4,
      messages: [
        ...toolOnlySummary.messages,
        {
          id: 'message-coach-1',
          role: 'coach' as const,
          content: 'Coach reply after tools',
          createdAtEpochSeconds: 4,
        },
      ],
    };

    const { rerender, result } = renderHook(
      ({ cachedSummary }) => useCoachChat({ apiBaseUrl: '', workoutId: '101', cachedSummary }),
      { initialProps: { cachedSummary: null as typeof toolOnlySummary | null } },
    );

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'coach_message',
            message: completeSummary.messages[2],
            summary: completeSummary,
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.messages.map((message) => message.content)).toContain('Coach reply after tools');
    });

    rerender({ cachedSummary: toolOnlySummary });

    expect(result.current.messages.map((message) => message.content)).toContain('Coach reply after tools');
  });

  it('recognizes the backend availability error sentinel', () => {
    expect(isAvailabilityRequiredChatError(availabilityRequiredChatError)).toBe(true);
    expect(isAvailabilityRequiredChatError('Availability must be configured before chatting with coach.')).toBe(true);
    expect(isAvailabilityRequiredChatError('other error')).toBe(false);
    expect(isAvailabilityRequiredChatError(null)).toBe(false);
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

  it('preserves app path prefixes in websocket urls', () => {
    expect(buildWorkoutSummaryWebSocketUrl('https://example.com/myapp', '101')).toBe(
      'wss://example.com/myapp/api/workout-summaries/101/ws',
    );
  });
});
