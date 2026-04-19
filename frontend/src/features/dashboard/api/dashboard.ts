import { get } from '../../../lib/httpClient';
import { dashboardRangeSchema, trainingLoadDashboardResponseSchema, type DashboardRange } from '../types';

export async function loadTrainingLoadDashboard(apiBaseUrl: string, range: DashboardRange | unknown) {
  const validatedRange = dashboardRangeSchema.parse(range);
  const data = await get(apiBaseUrl, `/api/dashboard/training-load?range=${validatedRange}`);
  return trainingLoadDashboardResponseSchema.parse(data);
}
