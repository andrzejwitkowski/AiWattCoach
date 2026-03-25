import {
  AuthenticationError,
  buildUrl,
  del,
  get,
  HttpError,
  post,
  put,
} from '../../../lib/httpClient';
import {
  createIntervalEventRequestSchema,
  intervalActivitiesResponseSchema,
  intervalActivitySchema,
  intervalEventSchema,
  intervalEventsResponseSchema,
  listEventsQuerySchema,
  updateActivityRequestSchema,
  updateIntervalEventRequestSchema,
  uploadActivityRequestSchema,
  uploadActivityResponseSchema,
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

export async function listActivities(apiBaseUrl: string, query: unknown) {
  const validated = listEventsQuerySchema.parse(query);
  const path = `/api/intervals/activities?${toQueryString(validated)}`;
  const data = await get(apiBaseUrl, path);
  return intervalActivitiesResponseSchema.parse(data);
}

export async function loadActivity(apiBaseUrl: string, activityId: string) {
  const data = await get(apiBaseUrl, `/api/intervals/activities/${activityId}`);
  return intervalActivitySchema.parse(data);
}

export async function uploadActivity(apiBaseUrl: string, data: unknown) {
  const validated = uploadActivityRequestSchema.parse(data);
  const result = await post<typeof validated, unknown>(apiBaseUrl, '/api/intervals/activities', validated);
  return uploadActivityResponseSchema.parse(result);
}

export async function updateActivity(apiBaseUrl: string, activityId: string, data: unknown) {
  const validated = updateActivityRequestSchema.parse(data);
  const result = await put<typeof validated, unknown>(
    apiBaseUrl,
    `/api/intervals/activities/${activityId}`,
    validated
  );
  return intervalActivitySchema.parse(result);
}

export async function deleteActivity(apiBaseUrl: string, activityId: string) {
  return del<void>(apiBaseUrl, `/api/intervals/activities/${activityId}`);
}

export async function downloadFit(apiBaseUrl: string, eventId: number): Promise<Uint8Array> {
  const path = `/api/intervals/events/${eventId}/download.fit`;
  const url = buildUrl(apiBaseUrl, path);

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
