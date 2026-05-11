import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import { buildCalendarCoachWebSocketUrl, useCalendarCoachChat } from './useCalendarCoachChat';

const coachApi = {
  getCurrentCalendarCoachConversation: vi.fn(),
  startNewCalendarCoachConversation: vi.fn(),
  getCalendarCoachConversation: vi.fn(),
  sendCalendarCoachMessage: vi.fn(),
};

vi.mock('../api/calendar', () => ({
  useCalendarCoachApi: () => coachApi,
  listCalendarLabels: vi.fn(),
  refreshCalendarView: vi.fn(),
}));

vi.mock('../../../lib/apiBaseUrl', () => ({
  useApiBaseUrl: () => '',
}));

class FakeWebSocket {
  static instances: FakeWebSocket[] = [];
  static OPEN = 1;
  static CLOSED = 3;
  static failNextConnection = false;

  public readyState = 1;
  private listeners = new Map<string, Set<(event?: MessageEvent) => void>>();

  constructor(public readonly url: string) {
    FakeWebSocket.instances.push(this);
    queueMicrotask(() => {
      if (FakeWebSocket.failNextConnection) {
        FakeWebSocket.failNextConnection = false;
        this.emit('error');
        return;
      }
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

const conversationFixture = {
  conversationId: 'conversation-1',
  surface: 'calendar' as const,
  status: 'active' as const,
  focus: 'overview' as const,
  createdAtEpochSeconds: 1,
  updatedAtEpochSeconds: 2,
};

const messageFixture = {
  id: 'message-1',
  role: 'coach' as const,
  content: 'Coach reply',
  createdAtEpochSeconds: 3,
};

afterEach(() => {
  vi.resetAllMocks();
  FakeWebSocket.instances = [];
  FakeWebSocket.failNextConnection = false;
  global.WebSocket = originalWebSocket;
  Object.defineProperty(window, 'location', {
    configurable: true,
    value: originalLocation,
  });
});

describe('useCalendarCoachChat', () => {
  it('loads current conversation and connects websocket when modal opens', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [messageFixture],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.conversation?.conversationId).toBe('conversation-1');
      expect(result.current.isConnected).toBe(true);
    });

    expect(FakeWebSocket.instances[0]?.url).toContain('/api/calendar/coach/conversations/conversation-1/ws');
  });

  it('ignores stale load responses after switching conversations', async () => {
    global.WebSocket = undefined as unknown as typeof WebSocket;

    const resolveLoadRef = { current: null as ((value: unknown) => void) | null };
    coachApi.getCurrentCalendarCoachConversation.mockImplementation(() => new Promise((resolve) => {
      resolveLoadRef.current = resolve;
    }));
    coachApi.startNewCalendarCoachConversation.mockResolvedValue({
      conversation: { ...conversationFixture, conversationId: 'conversation-2' },
      messages: [],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await act(async () => {
      await result.current.startNewConversation();
    });

    resolveLoadRef.current?.({
      conversation: conversationFixture,
      messages: [messageFixture],
    });

    await waitFor(() => {
      expect(result.current.conversation?.conversationId).toBe('conversation-2');
    });
    expect(result.current.messages).toHaveLength(0);
  });

  it('starts a new conversation on first send when none exists', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockRejectedValue(new HttpError(404, 'not found'));
    coachApi.startNewCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      await result.current.sendMessage('How does this week look?');
    });

    expect(coachApi.startNewCalendarCoachConversation).toHaveBeenCalled();
    expect(FakeWebSocket.instances[0]?.send).toHaveBeenCalledWith(
      JSON.stringify({ type: 'send_message', content: 'How does this week look?' }),
    );
    expect(result.current.messages[0]?.content).toBe('How does this week look?');
  });

  it('clears the current conversation when load returns 404 for the same conversation', async () => {
    global.WebSocket = undefined as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValueOnce({
      conversation: conversationFixture,
      messages: [messageFixture],
    }).mockRejectedValueOnce(new HttpError(404, 'not found'));

    const { result, rerender } = renderHook(({ isOpen }) => useCalendarCoachChat({ isOpen }), {
      initialProps: { isOpen: true },
    });

    await waitFor(() => {
      expect(result.current.conversation?.conversationId).toBe('conversation-1');
      expect(result.current.messages).toHaveLength(1);
    });

    rerender({ isOpen: false });
    rerender({ isOpen: true });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.conversation).toBeNull();
    expect(result.current.messages).toHaveLength(0);
  });

  it('appends websocket typing and coach reply updates', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    await act(async () => {
      await result.current.sendMessage('Need recovery advice');
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({ type: 'coach_typing' }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.isCoachTyping).toBe(true);
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'coach_message',
            message: messageFixture,
            conversation: conversationFixture,
            messages: [
              {
                id: 'message-user-1',
                role: 'user',
                content: 'Need recovery advice',
                createdAtEpochSeconds: 2,
              },
              messageFixture,
            ],
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.isCoachTyping).toBe(false);
      expect(result.current.messages.at(-1)?.content).toBe('Coach reply');
    });
  });

  it('appends streamed tool messages before the final calendar coach reply', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    await act(async () => {
      await result.current.sendMessage('Need recovery advice');
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'tool_message',
            message: {
              id: 'tool-1',
              role: 'tool',
              content: 'Tool call: lookupCalendar',
              toolCall: {
                id: 'tool-1',
                name: 'lookupCalendar',
                argumentsJson: '{"week":"2026-W18"}',
                argumentsPreview: 'week 2026-W18',
              },
              createdAtEpochSeconds: 3,
            },
          }),
        }),
      );
    });

    await waitFor(() => {
      const tool = result.current.messages.at(-1);
      expect(tool?.role).toBe('tool');
      expect(tool?.toolCall).toEqual({
        id: 'tool-1',
        name: 'lookupCalendar',
        argumentsJson: '{"week":"2026-W18"}',
        argumentsPreview: 'week 2026-W18',
      });
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'coach_message',
            message: messageFixture,
            conversation: conversationFixture,
            messages: [
              {
                id: 'message-user-1',
                role: 'user',
                content: 'Need recovery advice',
                createdAtEpochSeconds: 2,
              },
              {
                id: 'tool-1',
                role: 'tool',
                content: 'Tool call: lookupCalendar',
                toolCall: {
                  id: 'tool-1',
                  name: 'lookupCalendar',
                  argumentsJson: '{"week":"2026-W18"}',
                  argumentsPreview: 'week 2026-W18',
                },
                createdAtEpochSeconds: 3,
              },
              messageFixture,
            ],
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.messages.map((message) => message.role)).toEqual(['user', 'tool', 'coach']);
      const toolMessage = result.current.messages[1];
      expect(toolMessage.toolCall).toEqual({
        id: 'tool-1',
        name: 'lookupCalendar',
        argumentsJson: '{"week":"2026-W18"}',
        argumentsPreview: 'week 2026-W18',
      });
    });
  });

  it('calls onPlannedWorkoutUpdated when update_planned_workout tool message arrives', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [],
    });
    const onPlannedWorkoutUpdated = vi.fn();

    renderHook(() => useCalendarCoachChat({ isOpen: true, onPlannedWorkoutUpdated }));

    await waitFor(() => {
      expect(FakeWebSocket.instances).toHaveLength(1);
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'tool_message',
            message: {
              id: 'tool-update-1',
              role: 'tool',
              content: 'Tool call: update_planned_workout',
              toolCall: {
                id: 'tool-update-1',
                name: 'update_planned_workout',
                argumentsJson: '{"date":"2026-05-05","plannedWorkoutId":"pw-1","workoutDoc":"Warmup"}',
                argumentsPreview: 'replace pw-1 on 2026-05-05',
              },
              createdAtEpochSeconds: 3,
            },
          }),
        }),
      );
    });

    expect(onPlannedWorkoutUpdated).toHaveBeenCalledTimes(1);
  });

  it('deduplicates rapid new conversation requests', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockRejectedValue(new HttpError(404, 'not found'));
    coachApi.startNewCalendarCoachConversation.mockResolvedValue({
      conversation: { ...conversationFixture, conversationId: 'conversation-2' },
      messages: [],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      const first = result.current.startNewConversation();
      const second = result.current.startNewConversation();
      await Promise.all([first, second]);
    });

    expect(coachApi.startNewCalendarCoachConversation).toHaveBeenCalledTimes(1);
    expect(result.current.conversation?.conversationId).toBe('conversation-2');
  });

  it('creates a fresh empty transcript when starting a new conversation', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [messageFixture],
    });
    coachApi.startNewCalendarCoachConversation.mockResolvedValue({
      conversation: { ...conversationFixture, conversationId: 'conversation-2' },
      messages: [],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.messages).toHaveLength(1);
    });

    await act(async () => {
      await result.current.startNewConversation();
    });

    expect(result.current.conversation?.conversationId).toBe('conversation-2');
    expect(result.current.messages).toHaveLength(0);
  });

  it('ignores stale implicit conversation creation after a newer conversation is opened', async () => {
    global.WebSocket = undefined as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockRejectedValue(new HttpError(404, 'not found'));

    const resolveFirstCreateRef = { current: null as ((value: unknown) => void) | null };
    coachApi.startNewCalendarCoachConversation
      .mockImplementationOnce(() => new Promise((resolve) => {
        resolveFirstCreateRef.current = resolve;
      }))
      .mockResolvedValueOnce({
        conversation: { ...conversationFixture, conversationId: 'conversation-2' },
        messages: [],
      });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    const sendPromise = result.current.sendMessage('Implicit create');

    await waitFor(() => {
      expect(coachApi.startNewCalendarCoachConversation).toHaveBeenCalledTimes(1);
    });

    await act(async () => {
      await result.current.startNewConversation();
    });

    resolveFirstCreateRef.current?.({
      conversation: conversationFixture,
      messages: [],
    });

    await act(async () => {
      await sendPromise;
    });

    expect(result.current.conversation?.conversationId).toBe('conversation-2');
    expect(result.current.messages).toHaveLength(0);
  });

  it('shows error when starting a new conversation fails', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [],
    });
    coachApi.startNewCalendarCoachConversation.mockRejectedValue(new Error('Service unavailable'));

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    await act(async () => {
      await result.current.startNewConversation();
    });

    expect(result.current.error).toBe('Service unavailable');
    expect(result.current.conversation?.conversationId).toBe('conversation-1');
  });

  it('redirects to the landing page on auth failure', async () => {
    coachApi.getCurrentCalendarCoachConversation.mockRejectedValue(new AuthenticationError());
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, href: '/calendar' },
    });

    renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(window.location.href).toBe('/');
    });
  });

  it('falls back to REST send when websocket is unavailable', async () => {
    global.WebSocket = undefined as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [],
    });
    coachApi.sendCalendarCoachMessage.mockResolvedValue({
      conversation: conversationFixture,
      messages: [
        {
          id: 'message-user-1',
          role: 'user',
          content: 'Can we replan this week?',
          createdAtEpochSeconds: 2,
        },
        messageFixture,
      ],
      userMessage: {
        id: 'message-user-1',
        role: 'user',
        content: 'Can we replan this week?',
        createdAtEpochSeconds: 2,
      },
      coachMessage: messageFixture,
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.conversation?.conversationId).toBe('conversation-1');
    });

    await act(async () => {
      await result.current.sendMessage('Can we replan this week?');
    });

    expect(coachApi.sendCalendarCoachMessage).toHaveBeenCalledWith('conversation-1', { content: 'Can we replan this week?' });
    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[0]?.content).toBe('Can we replan this week?');
    expect(result.current.messages[1]?.content).toBe('Coach reply');
  });

  it('calls onPlannedWorkoutUpdated when REST fallback response contains update tool message', async () => {
    global.WebSocket = undefined as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [],
    });
    coachApi.sendCalendarCoachMessage.mockResolvedValue({
      conversation: conversationFixture,
      messages: [
        {
          id: 'message-user-1',
          role: 'user',
          content: 'Update this workout',
          createdAtEpochSeconds: 2,
        },
        {
          id: 'tool-update-1',
          role: 'tool',
          content: 'Tool call: update_planned_workout',
          toolCall: {
            id: 'tool-update-1',
            name: 'update_planned_workout',
            argumentsJson: '{"date":"2026-05-05","plannedWorkoutId":"pw-1","workoutDoc":"Warmup"}',
            argumentsPreview: 'replace pw-1 on 2026-05-05',
          },
          createdAtEpochSeconds: 3,
        },
        messageFixture,
      ],
      userMessage: {
        id: 'message-user-1',
        role: 'user',
        content: 'Update this workout',
        createdAtEpochSeconds: 2,
      },
      coachMessage: messageFixture,
    });
    const onPlannedWorkoutUpdated = vi.fn();

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true, onPlannedWorkoutUpdated }));

    await waitFor(() => {
      expect(result.current.conversation?.conversationId).toBe('conversation-1');
    });

    await act(async () => {
      await result.current.sendMessage('Update this workout');
    });

    expect(onPlannedWorkoutUpdated).toHaveBeenCalledTimes(1);
  });

  it('does not call onPlannedWorkoutUpdated for historical REST fallback update tool messages', async () => {
    global.WebSocket = undefined as unknown as typeof WebSocket;
    const historicalToolMessage = {
      id: 'tool-update-1',
      role: 'tool' as const,
      content: 'Tool call: update_planned_workout',
      toolCall: {
        id: 'tool-update-1',
        name: 'update_planned_workout',
        argumentsJson: '{"date":"2026-05-05","plannedWorkoutId":"pw-1","workoutDoc":"Warmup"}',
        argumentsPreview: 'replace pw-1 on 2026-05-05',
      },
      createdAtEpochSeconds: 3,
    };
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [historicalToolMessage],
    });
    coachApi.sendCalendarCoachMessage.mockResolvedValue({
      conversation: conversationFixture,
      messages: [
        historicalToolMessage,
        {
          id: 'message-user-2',
          role: 'user',
          content: 'Regular follow-up',
          createdAtEpochSeconds: 4,
        },
        messageFixture,
      ],
      userMessage: {
        id: 'message-user-2',
        role: 'user',
        content: 'Regular follow-up',
        createdAtEpochSeconds: 4,
      },
      coachMessage: messageFixture,
    });
    const onPlannedWorkoutUpdated = vi.fn();

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true, onPlannedWorkoutUpdated }));

    await waitFor(() => {
      expect(result.current.messages).toHaveLength(1);
    });

    await act(async () => {
      await result.current.sendMessage('Regular follow-up');
    });

    expect(onPlannedWorkoutUpdated).not.toHaveBeenCalled();
  });

  it('preserves app path prefixes in websocket urls', () => {
    expect(buildCalendarCoachWebSocketUrl('https://example.com/myapp', 'conversation-1')).toBe(
      'wss://example.com/myapp/api/calendar/coach/conversations/conversation-1/ws',
    );
  });

  it('keeps loaded conversation state when websocket connect fails during load', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    FakeWebSocket.failNextConnection = true;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [messageFixture],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.conversation?.conversationId).toBe('conversation-1');
      expect(result.current.messages).toHaveLength(1);
      expect(result.current.error).toBe('Unable to connect to the calendar coach right now.');
    });
  });

  it('keeps new conversation state when websocket connect fails after creating it', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockRejectedValue(new HttpError(404, 'not found'));
    coachApi.startNewCalendarCoachConversation.mockResolvedValue({
      conversation: { ...conversationFixture, conversationId: 'conversation-2' },
      messages: [],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    FakeWebSocket.failNextConnection = true;

    await act(async () => {
      await result.current.startNewConversation();
    });

    expect(result.current.conversation?.conversationId).toBe('conversation-2');
    expect(result.current.error).toBe('Unable to connect to the calendar coach right now.');
  });

  it('reloads current state when send returns not found', async () => {
    global.WebSocket = undefined as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [],
    });
    coachApi.sendCalendarCoachMessage.mockRejectedValue(new HttpError(404, 'not found'));
    coachApi.getCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [messageFixture],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.conversation?.conversationId).toBe('conversation-1');
    });

    await act(async () => {
      await result.current.sendMessage('Check latest state');
    });

    expect(coachApi.getCalendarCoachConversation).toHaveBeenCalledWith('conversation-1');
    expect(result.current.messages).toHaveLength(1);
  });

  it('reloads the just-created conversation when first send returns not found', async () => {
    global.WebSocket = undefined as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockRejectedValue(new HttpError(404, 'not found'));
    coachApi.startNewCalendarCoachConversation.mockResolvedValue({
      conversation: { ...conversationFixture, conversationId: 'conversation-2' },
      messages: [],
    });
    coachApi.sendCalendarCoachMessage.mockRejectedValue(new HttpError(404, 'not found'));
    coachApi.getCalendarCoachConversation.mockResolvedValue({
      conversation: { ...conversationFixture, conversationId: 'conversation-2' },
      messages: [messageFixture],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      await result.current.sendMessage('Check latest state after create');
    });

    expect(coachApi.getCalendarCoachConversation).toHaveBeenCalledWith('conversation-2');
    expect(result.current.conversation?.conversationId).toBe('conversation-2');
    expect(result.current.messages).toHaveLength(1);
  });

  it('ignores stale websocket events immediately after switching conversations', async () => {
    global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockResolvedValue({
      conversation: conversationFixture,
      messages: [messageFixture],
    });
    coachApi.startNewCalendarCoachConversation.mockResolvedValue({
      conversation: { ...conversationFixture, conversationId: 'conversation-2' },
      messages: [],
    });

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.conversation?.conversationId).toBe('conversation-1');
    });

    await act(async () => {
      await result.current.startNewConversation();
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'coach_message',
            message: messageFixture,
            conversation: conversationFixture,
            messages: [messageFixture],
          }),
        }),
      );
    });

    expect(result.current.conversation?.conversationId).toBe('conversation-2');
    expect(result.current.messages).toHaveLength(0);
  });

  it('ignores stale rest send responses after switching conversations', async () => {
    global.WebSocket = undefined as unknown as typeof WebSocket;
    coachApi.getCurrentCalendarCoachConversation.mockRejectedValue(new HttpError(404, 'not found'));
    coachApi.startNewCalendarCoachConversation
      .mockResolvedValueOnce({
        conversation: { ...conversationFixture, conversationId: 'conversation-1' },
        messages: [],
      })
      .mockResolvedValueOnce({
        conversation: { ...conversationFixture, conversationId: 'conversation-2' },
        messages: [],
      });

    const resolveSendRef = { current: null as ((value: unknown) => void) | null };
    coachApi.sendCalendarCoachMessage.mockImplementation(() => new Promise((resolve) => {
      resolveSendRef.current = resolve;
    }));

    const { result } = renderHook(() => useCalendarCoachChat({ isOpen: true }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    const sendPromise = result.current.sendMessage('First conversation message');

    await waitFor(() => {
      expect(coachApi.sendCalendarCoachMessage).toHaveBeenCalledWith('conversation-1', { content: 'First conversation message' });
    });

    await act(async () => {
      await result.current.startNewConversation();
    });

    resolveSendRef.current?.({
      conversation: { ...conversationFixture, conversationId: 'conversation-1' },
      messages: [messageFixture],
    });
    await act(async () => {
      await sendPromise;
    });

    expect(result.current.conversation?.conversationId).toBe('conversation-2');
    expect(result.current.messages).toHaveLength(0);
  });
});
