import { get, post } from '../../../lib/httpClient';
import {
  mesoCycleCalendarDaySchema,
  mesoCycleOperationSchema,
  mesoCycleStatusSchema,
} from '../types';

export async function loadMesoCycleStatus(apiBaseUrl: string) {
  const data = await get(apiBaseUrl, '/api/meso-cycle/status');
  return mesoCycleStatusSchema.parse(data);
}

export async function loadMesoCycleCalendar(apiBaseUrl: string, from: string, to: string) {
  const data = await get(
    apiBaseUrl,
    `/api/meso-cycle/calendar?from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}`,
  );
  return mesoCycleCalendarDaySchema.array().parse(data);
}

export async function generateMesoCyclePlan(apiBaseUrl: string) {
  const data = await post<Record<string, never>, unknown>(apiBaseUrl, '/api/meso-cycle/generate', {});
  return mesoCycleOperationSchema.parse(data);
}
