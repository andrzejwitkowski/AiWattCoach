import { renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { ApiBaseUrlProvider } from '../../../lib/apiBaseUrl';
import { useCalendarCoachApi, listCalendarLabels } from './calendar';
import { createFetchMock, useFetchMock } from '../../intervals/api/testHelpers';

function wrapper(apiBaseUrl: string) {
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <ApiBaseUrlProvider value={apiBaseUrl}>{children}</ApiBaseUrlProvider>;
  };
}

describe('calendar api', () => {
  it('loads race labels grouped by date', async () => {
    const fetchMock = useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            labelsByDate: {
              '2026-09-12': {
                'race:race-1': {
                  kind: 'race',
                  title: 'Race Gravel Attack',
                  subtitle: '120 km • Kat. A',
                  payload: {
                    raceId: 'race-1',
                    date: '2026-09-12',
                    name: 'Gravel Attack',
                    distanceMeters: 120000,
                    discipline: 'gravel',
                    priority: 'A',
                    syncStatus: 'synced',
                    linkedIntervalsEventId: 41,
                  },
                },
              },
            },
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    const result = await listCalendarLabels('', { oldest: '2026-09-01', newest: '2026-09-30' });

    expect(fetchMock).toHaveBeenCalledWith('/api/calendar/labels?oldest=2026-09-01&newest=2026-09-30', {
      method: 'GET',
      headers: {
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: undefined,
    });
    const raceLabel = result.labelsByDate['2026-09-12']?.['race:race-1'];

    expect(raceLabel?.kind).toBe('race');
    if (raceLabel?.kind === 'race') {
      expect(raceLabel.payload.raceId).toBe('race-1');
    }
  });

  it('loads current calendar coach conversation', async () => {
    useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            conversation: {
              conversationId: 'conversation-1',
              surface: 'calendar',
              status: 'active',
              focus: 'overview',
              createdAtEpochSeconds: 1,
              updatedAtEpochSeconds: 2,
            },
            messages: [],
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    const { result } = renderHook(() => useCalendarCoachApi(), { wrapper: wrapper('') });

    const conversation = await result.current.getCurrentCalendarCoachConversation();

    expect(conversation.conversation.conversationId).toBe('conversation-1');
  });

  it('starts a new calendar coach conversation', async () => {
    const fetchMock = useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            conversation: {
              conversationId: 'conversation-2',
              surface: 'calendar',
              status: 'active',
              focus: 'overview',
              createdAtEpochSeconds: 1,
              updatedAtEpochSeconds: 1,
            },
            messages: [],
          }),
          { status: 201, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    const { result } = renderHook(() => useCalendarCoachApi(), { wrapper: wrapper('') });

    const conversation = await result.current.startNewCalendarCoachConversation();

    expect(fetchMock).toHaveBeenCalledWith('/api/calendar/coach/conversations', expect.objectContaining({ method: 'POST' }));
    expect(conversation.conversation.conversationId).toBe('conversation-2');
  });

  it('loads a specific calendar coach conversation', async () => {
    useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            conversation: {
              conversationId: 'conversation-2',
              surface: 'calendar',
              status: 'active',
              focus: 'overview',
              createdAtEpochSeconds: 1,
              updatedAtEpochSeconds: 2,
            },
            messages: [],
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    const { result } = renderHook(() => useCalendarCoachApi(), { wrapper: wrapper('') });

    const conversation = await result.current.getCalendarCoachConversation('conversation-2');

    expect(conversation.conversation.conversationId).toBe('conversation-2');
  });

  it('sends a calendar coach message', async () => {
    const fetchMock = useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            conversation: {
              conversationId: 'conversation-1',
              surface: 'calendar',
              status: 'active',
              focus: 'overview',
              createdAtEpochSeconds: 1,
              updatedAtEpochSeconds: 3,
            },
            messages: [
              {
                id: 'message-user-1',
                role: 'user',
                content: 'How is the week balanced?',
                createdAtEpochSeconds: 2,
              },
              {
                id: 'message-coach-2',
                role: 'coach',
                content: 'The week is front-loaded.',
                createdAtEpochSeconds: 3,
              },
            ],
            userMessage: {
              id: 'message-user-1',
              role: 'user',
              content: 'How is the week balanced?',
              createdAtEpochSeconds: 2,
            },
            coachMessage: {
              id: 'message-coach-2',
              role: 'coach',
              content: 'The week is front-loaded.',
              createdAtEpochSeconds: 3,
            },
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    const { result } = renderHook(() => useCalendarCoachApi(), { wrapper: wrapper('') });

    const response = await result.current.sendCalendarCoachMessage('conversation-1', { content: 'How is the week balanced?' });

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/calendar/coach/conversations/conversation-1/messages',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ content: 'How is the week balanced?' }),
      }),
    );
    expect(response.coachMessage.content).toBe('The week is front-loaded.');
  });
});
