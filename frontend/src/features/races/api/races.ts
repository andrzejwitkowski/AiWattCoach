import { del, get, post, put } from '../../../lib/httpClient';
import { listRacesQuerySchema, raceDtoSchema, racesResponseSchema, upsertRaceRequestSchema } from '../types';

function toQueryString(params: Record<string, string>): string {
  const searchParams = new URLSearchParams(params);
  return searchParams.toString();
}

export async function listRaces(apiBaseUrl: string, query: unknown) {
  const validated = listRacesQuerySchema.parse(query);
  const path = `/api/races?${toQueryString(validated)}`;
  const data = await get(apiBaseUrl, path);
  return racesResponseSchema.parse(data);
}

export async function createRace(apiBaseUrl: string, body: unknown) {
  const validated = upsertRaceRequestSchema.parse(body);
  const data = await post<typeof validated, unknown>(apiBaseUrl, '/api/races', validated);
  return raceDtoSchema.parse(data);
}

export async function getRace(apiBaseUrl: string, raceId: string) {
  const data = await get(apiBaseUrl, `/api/races/${encodeURIComponent(raceId)}`);
  return raceDtoSchema.parse(data);
}

export async function updateRace(apiBaseUrl: string, raceId: string, body: unknown) {
  const validated = upsertRaceRequestSchema.parse(body);
  const data = await put<typeof validated, unknown>(apiBaseUrl, `/api/races/${encodeURIComponent(raceId)}`, validated);
  return raceDtoSchema.parse(data);
}

export async function deleteRace(apiBaseUrl: string, raceId: string) {
  return del<void>(apiBaseUrl, `/api/races/${encodeURIComponent(raceId)}`);
}
