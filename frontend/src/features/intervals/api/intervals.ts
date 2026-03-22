import { AuthenticationError, del, get, HttpError, post, put } from '../../../lib/httpClient';
import {
  createIntervalEventRequestSchema,
  intervalEventSchema,
  intervalEventsResponseSchema,
  listEventsQuerySchema,
  updateIntervalEventRequestSchema,
} from '../types';

function toQueryString(params: Record<string, string>): string {
  const searchParams = new URLSearchParams(params);
  return searchParams.toString();
}

export async function listEvents(apiBaseUrl: string, query: unknown) {
  const validated = listEventsQuerySchema.parse(query);
  const path = `/api/intervals/events?${toQueryString(validated)}`;
  const data = await get(apiBaseUrl, path);
  return intervalEventsResponseSchema.parse(data);
}

export async function loadEvent(apiBaseUrl: string, eventId: number) {
  const data = await get(apiBaseUrl, `/api/intervals/events/${eventId}`);
  return intervalEventSchema.parse(data);
}

export async function createEvent(apiBaseUrl: string, data: unknown) {
  const validated = createIntervalEventRequestSchema.parse(data);
  const result = await post<typeof validated, unknown>(apiBaseUrl, '/api/intervals/events', validated);
  return intervalEventSchema.parse(result);
}

export async function updateEvent(apiBaseUrl: string, eventId: number, data: unknown) {
  const validated = updateIntervalEventRequestSchema.parse(data);
  const result = await put<typeof validated, unknown>(
    apiBaseUrl,
    `/api/intervals/events/${eventId}`,
    validated
  );
  return intervalEventSchema.parse(result);
}

export async function deleteEvent(apiBaseUrl: string, eventId: number) {
  return del<void>(apiBaseUrl, `/api/intervals/events/${eventId}`);
}

export async function downloadFit(apiBaseUrl: string, eventId: number): Promise<Uint8Array> {
  const base = apiBaseUrl.endsWith('/') ? apiBaseUrl.slice(0, -1) : apiBaseUrl;
  const path = `/api/intervals/events/${eventId}/download.fit`;
  const url = base ? `${base}${path}` : path;

  const response = await fetch(url, {
    method: 'GET',
    headers: {
      Accept: 'application/octet-stream',
    },
    credentials: 'include',
  });

  if (response.status === 401) {
    throw new AuthenticationError();
  }

  if (!response.ok) {
    throw new HttpError(response.status, `GET ${path} failed: ${response.status}`);
  }

  return new Uint8Array(await response.arrayBuffer());
}
