import { z } from 'zod';

import { get, post } from '../../../lib/httpClient';
import { listEventsQuerySchema } from '../../intervals/types';
import { calendarLabelsResponseSchema } from '../types';

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
