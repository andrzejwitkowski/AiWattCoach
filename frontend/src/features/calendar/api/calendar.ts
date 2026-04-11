import { get } from '../../../lib/httpClient';
import { listEventsQuerySchema } from '../../intervals/types';
import { calendarLabelsResponseSchema } from '../types';

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
