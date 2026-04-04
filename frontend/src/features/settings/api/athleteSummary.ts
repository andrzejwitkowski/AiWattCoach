import { get, post } from '../../../lib/httpClient';
import { athleteSummaryResponseSchema } from '../types';

export async function loadAthleteSummary(apiBaseUrl: string) {
  const data = await get(apiBaseUrl, '/api/athlete-summary');
  return athleteSummaryResponseSchema.parse(data);
}

export async function generateAthleteSummary(apiBaseUrl: string) {
  const data = await post<Record<string, never>, unknown>(apiBaseUrl, '/api/athlete-summary/generate', {});
  return athleteSummaryResponseSchema.parse(data);
}
