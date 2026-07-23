import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { HttpError } from '../../../lib/httpClient';
import {
  createWorkoutSummary,
  getWorkoutSummary,
  sendWorkoutSummaryMessage,
  updateWorkoutSummaryRpe,
} from '../api/workoutSummary';
import { availabilityRequiredChatError, useCoachChat } from './useCoachChat';
import {
  FakeWebSocket,
  installFakeWebSocket,
  resetCoachChatTestEnvironment,
  summaryFixture,
} from './useCoachChat.testUtils';
import type { SendMessageResponse } from '../types';

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

describe('useCoachChat sendMessage', () => {
  it('creates a summary on first send when one does not exist', async () => {
    installFakeWebSocket();
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
    expect(result.current.progressState).toBe('awaiting-reply');
  });

  it('loads existing summary after create conflict', async () => {
    installFakeWebSocket();
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
    expect(getWorkoutSummary.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(result.current.summary?.rpe).toBe(5);
  });

  it('does not create chat session before rpe is chosen', async () => {
    installFakeWebSocket();
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
    installFakeWebSocket();
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
            error: availabilityRequiredChatError,
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.error).toBe(availabilityRequiredChatError);
    });

    expect(result.current.progressState).toBe('idle');
    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0]?.content).toBe('Need feedback');
  });

  it('falls back to REST send when websocket connection fails during send', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);
    let resolveFallback: ((value: SendMessageResponse) => void) | undefined;
    vi.mocked(sendWorkoutSummaryMessage).mockImplementation(() => new Promise((resolve) => {
      resolveFallback = resolve;
    }));

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    FakeWebSocket.failNextConnection = true;
    act(() => {
      FakeWebSocket.instances[0]?.emit('error');
    });

    await waitFor(() => {
      expect(result.current.isConnected).toBe(false);
    });

    let sendPromise: Promise<boolean> | undefined;
    await act(async () => {
      sendPromise = result.current.sendMessage('Need feedback');
    });

    expect(result.current.error).toBeNull();

    act(() => {
      resolveFallback?.({
        summary: {
          ...summaryFixture,
          messages: [
            {
              id: 'message-user-1',
              role: 'user',
              content: 'Need feedback',
              createdAtEpochSeconds: 2,
            },
            {
              id: 'message-coach-1',
              role: 'coach',
              content: 'Coach reply',
              createdAtEpochSeconds: 3,
            },
          ],
        },
        userMessage: {
          id: 'message-user-1',
          role: 'user',
          content: 'Need feedback',
          createdAtEpochSeconds: 2,
        },
        coachMessage: {
          id: 'message-coach-1',
          role: 'coach',
          content: 'Coach reply',
          createdAtEpochSeconds: 3,
        },
      });
    });

    await act(async () => {
      const sent = await sendPromise;
      expect(sent).toBe(true);
    });

    expect(sendWorkoutSummaryMessage).toHaveBeenCalledWith('', '101', { content: 'Need feedback' });
    expect(result.current.messages.map((message) => message.content)).toEqual([
      'Need feedback',
      'Coach reply',
    ]);
    expect(result.current.error).toBeNull();
    expect(result.current.progressState).toBe('idle');
  });

  it('keeps awaiting reply state active after a system message until coach reply arrives', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    await act(async () => {
      await result.current.sendMessage('Need feedback');
    });

    expect(result.current.progressState).toBe('awaiting-reply');

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'system_message',
            content: 'Generating summary context.',
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.messages.at(-1)?.content).toBe('Generating summary context.');
    });

    expect(result.current.progressState).toBe('awaiting-reply');

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'coach_message',
            message: {
              id: 'message-2',
              role: 'coach',
              content: 'Coach reply',
              createdAtEpochSeconds: 3,
            },
            summary: {
              ...summaryFixture,
              messages: [
                {
                  id: 'temp-user',
                  role: 'user',
                  content: 'Need feedback',
                  createdAtEpochSeconds: 2,
                },
                {
                  id: 'message-2',
                  role: 'coach',
                  content: 'Coach reply',
                  createdAtEpochSeconds: 3,
                },
              ],
            },
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.progressState).toBe('idle');
    });
  });

  it('appends streamed tool messages before the final coach reply', async () => {
    installFakeWebSocket();
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
            type: 'tool_message',
            message: {
              id: 'tool-1',
              role: 'tool',
              content: 'Tool call: lookupCalendar',
              toolCall: {
                id: 'tool-1',
                name: 'lookupCalendar',
                argumentsJson: '{"date":"2026-05-02"}',
                argumentsPreview: '1 dated day from 2026-05-02 to 2026-05-02',
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
        argumentsJson: '{"date":"2026-05-02"}',
        argumentsPreview: '1 dated day from 2026-05-02 to 2026-05-02',
      });
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'coach_message',
            message: {
              id: 'message-2',
              role: 'coach',
              content: 'Coach reply',
              createdAtEpochSeconds: 4,
            },
            summary: {
              ...summaryFixture,
              messages: [
                {
                  id: 'temp-user',
                  role: 'user',
                  content: 'Need feedback',
                  createdAtEpochSeconds: 2,
                },
                {
                  id: 'tool-1',
                  role: 'tool',
                  content: 'Tool call: lookupCalendar',
                  toolCall: {
                    id: 'tool-1',
                    name: 'lookupCalendar',
                    argumentsJson: '{"date":"2026-05-02"}',
                    argumentsPreview: '1 dated day from 2026-05-02 to 2026-05-02',
                  },
                  createdAtEpochSeconds: 3,
                },
                {
                  id: 'message-2',
                  role: 'coach',
                  content: 'Coach reply',
                  createdAtEpochSeconds: 4,
                },
              ],
            },
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
        argumentsJson: '{"date":"2026-05-02"}',
        argumentsPreview: '1 dated day from 2026-05-02 to 2026-05-02',
      });
    });
  });

  it('stores coach questionnaire data from websocket replies', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    act(() => {
      FakeWebSocket.instances[0]?.emit(
        'message',
        new MessageEvent('message', {
          data: JSON.stringify({
            type: 'coach_message',
            message: {
              id: 'message-2',
              role: 'coach',
              content: 'Tell me what held you back the most.',
              questions: [
                {
                  id: 'limiter',
                  question: 'What limited you most today?',
                  answers: ['Legs', 'Breathing', 'Fueling'],
                  freeTextLabel: 'Add context if needed',
                },
              ],
              createdAtEpochSeconds: 3,
            },
            summary: {
              ...summaryFixture,
              messages: [
                {
                  id: 'message-2',
                  role: 'coach',
                  content: 'Tell me what held you back the most.',
                  questions: [
                    {
                      id: 'limiter',
                      question: 'What limited you most today?',
                      answers: ['Legs', 'Breathing', 'Fueling'],
                      freeTextLabel: 'Add context if needed',
                    },
                  ],
                  createdAtEpochSeconds: 3,
                },
              ],
            },
          }),
        }),
      );
    });

    await waitFor(() => {
      expect(result.current.messages).toHaveLength(1);
      expect(result.current.messages[0]?.questions).toEqual([
        {
          id: 'limiter',
          question: 'What limited you most today?',
          answers: ['Legs', 'Breathing', 'Fueling'],
          freeTextLabel: 'Add context if needed',
        },
      ]);
    });

    expect(result.current.progressState).toBe('idle');
    expect(result.current.hasConversation).toBe(true);
  });

  it('persists draft rpe before first chat message', async () => {
    installFakeWebSocket();
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

  it('sets awaiting-reply immediately and ignores a concurrent send', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockRejectedValue(new HttpError(404, 'not found'));
    let resolveCreate: ((value: typeof summaryFixture) => void) | undefined;
    vi.mocked(createWorkoutSummary).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveCreate = resolve;
        }),
    );

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    act(() => {
      result.current.setDraftRpe(5);
    });

    let firstSend: Promise<boolean> | undefined;
    let secondSend: Promise<boolean> | undefined;
    await act(async () => {
      firstSend = result.current.sendMessage('Legs felt strong');
      secondSend = result.current.sendMessage('Legs felt strong');
    });

    expect(result.current.progressState).toBe('awaiting-reply');

    await act(async () => {
      resolveCreate?.({ ...summaryFixture, rpe: 5 });
      await expect(firstSend).resolves.toBe(true);
      await expect(secondSend).resolves.toBe(false);
    });

    expect(createWorkoutSummary).toHaveBeenCalledTimes(1);
    expect(FakeWebSocket.instances[0]?.send).toHaveBeenCalledTimes(1);
  });

  it('recovers a persisted coach reply via poll when websocket never emits coach_message', async () => {
    installFakeWebSocket();
    let returnPersistedReply = false;
    vi.mocked(getWorkoutSummary).mockImplementation(async () => {
      if (!returnPersistedReply) {
        return summaryFixture;
      }

      return {
        ...summaryFixture,
        updatedAtEpochSeconds: 99,
        messages: [
          {
            id: 'message-user-1',
            role: 'user',
            content: 'Need feedback',
            createdAtEpochSeconds: 2,
          },
          {
            id: 'message-coach-1',
            role: 'coach',
            content: 'Recovered coach reply',
            createdAtEpochSeconds: 3,
          },
        ],
      };
    });

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    await act(async () => {
      await result.current.sendMessage('Need feedback');
    });

    expect(result.current.progressState).toBe('awaiting-reply');
    expect(FakeWebSocket.instances[0]?.send).toHaveBeenCalled();

    returnPersistedReply = true;

    await waitFor(
      () => {
        expect(result.current.progressState).toBe('idle');
        expect(result.current.isCoachTyping).toBe(false);
        expect(result.current.messages.map((message) => message.content)).toEqual([
          'Need feedback',
          'Recovered coach reply',
        ]);
      },
      { timeout: 4000 },
    );
  });

  it('keeps awaiting-reply after websocket close and still recovers via poll', async () => {
    installFakeWebSocket();
    let returnPersistedReply = false;
    vi.mocked(getWorkoutSummary).mockImplementation(async () => {
      if (!returnPersistedReply) {
        return summaryFixture;
      }

      return {
        ...summaryFixture,
        updatedAtEpochSeconds: 99,
        messages: [
          {
            id: 'message-user-1',
            role: 'user',
            content: 'Need feedback',
            createdAtEpochSeconds: 2,
          },
          {
            id: 'message-coach-1',
            role: 'coach',
            content: 'Recovered after close',
            createdAtEpochSeconds: 3,
          },
        ],
      };
    });

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    await act(async () => {
      await result.current.sendMessage('Need feedback');
    });

    expect(result.current.progressState).toBe('awaiting-reply');

    act(() => {
      FakeWebSocket.instances[0]?.close();
    });

    await waitFor(() => {
      expect(result.current.isConnected).toBe(false);
    });

    expect(result.current.progressState).toBe('awaiting-reply');

    returnPersistedReply = true;

    await waitFor(
      () => {
        expect(result.current.progressState).toBe('idle');
        expect(result.current.messages.map((message) => message.content)).toEqual([
          'Need feedback',
          'Recovered after close',
        ]);
      },
      { timeout: 4000 },
    );
  });

  it('keeps send lock through websocket connect failure into REST fallback', async () => {
    installFakeWebSocket();
    vi.mocked(getWorkoutSummary).mockResolvedValue(summaryFixture);
    let resolveFallback: ((value: SendMessageResponse) => void) | undefined;
    vi.mocked(sendWorkoutSummaryMessage).mockImplementation(() => new Promise((resolve) => {
      resolveFallback = resolve;
    }));

    const { result } = renderHook(() => useCoachChat({ apiBaseUrl: '', workoutId: '101' }));

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    FakeWebSocket.failNextConnection = true;
    act(() => {
      FakeWebSocket.instances[0]?.emit('error');
    });

    await waitFor(() => {
      expect(result.current.isConnected).toBe(false);
    });

    let sendPromise: Promise<boolean> | undefined;
    let concurrentSend: Promise<boolean> | undefined;
    await act(async () => {
      sendPromise = result.current.sendMessage('Need feedback');
    });

    expect(result.current.progressState).toBe('awaiting-reply');

    await act(async () => {
      concurrentSend = result.current.sendMessage('Need feedback');
      await expect(concurrentSend).resolves.toBe(false);
    });

    act(() => {
      resolveFallback?.({
        summary: {
          ...summaryFixture,
          messages: [
            {
              id: 'message-user-1',
              role: 'user',
              content: 'Need feedback',
              createdAtEpochSeconds: 2,
            },
            {
              id: 'message-coach-1',
              role: 'coach',
              content: 'Coach reply',
              createdAtEpochSeconds: 3,
            },
          ],
        },
        userMessage: {
          id: 'message-user-1',
          role: 'user',
          content: 'Need feedback',
          createdAtEpochSeconds: 2,
        },
        coachMessage: {
          id: 'message-coach-1',
          role: 'coach',
          content: 'Coach reply',
          createdAtEpochSeconds: 3,
        },
      });
    });

    await act(async () => {
      await expect(sendPromise).resolves.toBe(true);
    });

    expect(sendWorkoutSummaryMessage).toHaveBeenCalledTimes(1);
    expect(result.current.progressState).toBe('idle');
  });
});
