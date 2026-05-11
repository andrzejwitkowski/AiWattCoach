import { useCallback, useMemo } from 'react';
import { z } from 'zod';

import { useApiBaseUrl } from '../../../lib/apiBaseUrl';
import { get, post } from '../../../lib/httpClient';
import { listEventsQuerySchema } from '../../intervals/types';
import {
  calendarCoachConversationResponseSchema,
  calendarCoachSendMessageRequestSchema,
  calendarCoachSendMessageResponseSchema,
  calendarLabelsResponseSchema,
} from '../types';

const manualCalendarRefreshResponseSchema = z.object({
  oldest: z.string(),
  newest: z.string(),
  rebuiltEntryCount: z.number().int().nonnegative(),
});

function toQueryString(params: Record<string, string>): string {
  const searchParams = new URLSearchParams(params);
  return searchParams.toString();
}

export async function listCalendarLabels(apiBaseUrl: string, query: unknown) {
  const validated = listEventsQuerySchema.parse(query);
  const path = `/api/calendar/labels?${toQueryString(validated)}`;
  const data = await get(apiBaseUrl, path);
  return calendarLabelsResponseSchema.parse(data);
}

export async function refreshCalendarView(apiBaseUrl: string) {
  const data = await post<undefined, unknown>(apiBaseUrl, '/api/calendar/refresh');
  return manualCalendarRefreshResponseSchema.parse(data);
}

export function useCalendarCoachApi() {
  const apiBaseUrl = useApiBaseUrl();

  const getCurrentCalendarCoachConversation = useCallback(async () => {
    const data = await get(apiBaseUrl, '/api/calendar/coach/current');
    return calendarCoachConversationResponseSchema.parse(data);
  }, [apiBaseUrl]);

  const startNewCalendarCoachConversation = useCallback(async () => {
    const data = await post<undefined, unknown>(apiBaseUrl, '/api/calendar/coach/conversations');
    return calendarCoachConversationResponseSchema.parse(data);
  }, [apiBaseUrl]);

  const getCalendarCoachConversation = useCallback(
    async (conversationId: string) => {
      const data = await get(apiBaseUrl, `/api/calendar/coach/conversations/${conversationId}`);
      return calendarCoachConversationResponseSchema.parse(data);
    },
    [apiBaseUrl],
  );

  const sendCalendarCoachMessage = useCallback(
    async (conversationId: string, payload: unknown) => {
      const validated = calendarCoachSendMessageRequestSchema.parse(payload);
      const data = await post<typeof validated, unknown>(
        apiBaseUrl,
        `/api/calendar/coach/conversations/${conversationId}/messages`,
        validated,
        { timeoutMs: 600_000 }, // defensive upper bound for long-running LLM tool loops
      );
      return calendarCoachSendMessageResponseSchema.parse(data);
    },
    [apiBaseUrl],
  );

  return useMemo(() => ({
    getCurrentCalendarCoachConversation,
    startNewCalendarCoachConversation,
    getCalendarCoachConversation,
    sendCalendarCoachMessage,
  }), [
    getCurrentCalendarCoachConversation,
    startNewCalendarCoachConversation,
    getCalendarCoachConversation,
    sendCalendarCoachMessage,
  ]);
}
