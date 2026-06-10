import { useCallback, useMemo } from 'react';

import { useApiBaseUrl } from '../../../lib/apiBaseUrl';
import { del, get, post, put } from '../../../lib/httpClient';
import {
  listPlannedRestDaysQuerySchema,
  plannedRestDaySchema,
  plannedRestDaysResponseSchema,
  upsertPlannedRestDayRequestSchema,
} from '../types';

function toQueryString(params: Record<string, string>): string {
  return new URLSearchParams(params).toString();
}

export async function listPlannedRestDays(apiBaseUrl: string, query: unknown) {
  const validated = listPlannedRestDaysQuerySchema.parse(query);
  const path = `/api/planned-rest-days?${toQueryString(validated)}`;
  const data = await get(apiBaseUrl, path);
  return plannedRestDaysResponseSchema.parse(data);
}

export async function createPlannedRestDay(apiBaseUrl: string, body: unknown) {
  const validated = upsertPlannedRestDayRequestSchema.parse(body);
  const data = await post<typeof validated, unknown>(apiBaseUrl, '/api/planned-rest-days', validated);
  return plannedRestDaySchema.parse(data);
}

export async function getPlannedRestDay(apiBaseUrl: string, plannedRestDayId: string) {
  const data = await get(apiBaseUrl, `/api/planned-rest-days/${encodeURIComponent(plannedRestDayId)}`);
  return plannedRestDaySchema.parse(data);
}

export async function updatePlannedRestDay(apiBaseUrl: string, plannedRestDayId: string, body: unknown) {
  const validated = upsertPlannedRestDayRequestSchema.parse(body);
  const data = await put<typeof validated, unknown>(
    apiBaseUrl,
    `/api/planned-rest-days/${encodeURIComponent(plannedRestDayId)}`,
    validated,
  );
  return plannedRestDaySchema.parse(data);
}

export async function deletePlannedRestDay(apiBaseUrl: string, plannedRestDayId: string) {
  return del<void>(apiBaseUrl, `/api/planned-rest-days/${encodeURIComponent(plannedRestDayId)}`);
}

export function usePlannedRestDaysApi() {
  const apiBaseUrl = useApiBaseUrl();

  const list = useCallback(
    (query: unknown) => listPlannedRestDays(apiBaseUrl, query),
    [apiBaseUrl],
  );

  const create = useCallback(
    (body: unknown) => createPlannedRestDay(apiBaseUrl, body),
    [apiBaseUrl],
  );

  const getById = useCallback(
    (plannedRestDayId: string) => getPlannedRestDay(apiBaseUrl, plannedRestDayId),
    [apiBaseUrl],
  );

  const update = useCallback(
    (plannedRestDayId: string, body: unknown) =>
      updatePlannedRestDay(apiBaseUrl, plannedRestDayId, body),
    [apiBaseUrl],
  );

  const remove = useCallback(
    (plannedRestDayId: string) => deletePlannedRestDay(apiBaseUrl, plannedRestDayId),
    [apiBaseUrl],
  );

  return useMemo(
    () => ({
      listPlannedRestDays: list,
      createPlannedRestDay: create,
      getPlannedRestDay: getById,
      updatePlannedRestDay: update,
      deletePlannedRestDay: remove,
    }),
    [create, getById, list, remove, update],
  );
}
