import { useCallback, useMemo } from 'react';

import { useApiBaseUrl } from '../../lib/apiBaseUrl';
import { get } from '../../lib/httpClient';
import { adminPromptPreviewResponseSchema, type AdminPromptPreviewResponse } from './types';

function buildPath(
  userId: string,
  surface: 'post-workout' | 'calendar-coach' | 'meso-cycle',
  date: string,
) {
  const encodedUserId = encodeURIComponent(userId);
  const query = new URLSearchParams({ date });
  return `/api/admin/users/${encodedUserId}/prompt-preview/${surface}?${query.toString()}`;
}

export async function loadAdminPostWorkoutPromptPreview(
  apiBaseUrl: string,
  userId: string,
  date: string,
): Promise<AdminPromptPreviewResponse> {
  const data = await get(apiBaseUrl, buildPath(userId, 'post-workout', date));
  return adminPromptPreviewResponseSchema.parse(data);
}

export async function loadAdminCalendarCoachPromptPreview(
  apiBaseUrl: string,
  userId: string,
  date: string,
): Promise<AdminPromptPreviewResponse> {
  const data = await get(apiBaseUrl, buildPath(userId, 'calendar-coach', date));
  return adminPromptPreviewResponseSchema.parse(data);
}

export async function loadAdminMesoCyclePromptPreview(
  apiBaseUrl: string,
  userId: string,
  date: string,
): Promise<AdminPromptPreviewResponse> {
  const data = await get(apiBaseUrl, buildPath(userId, 'meso-cycle', date));
  return adminPromptPreviewResponseSchema.parse(data);
}

export function useAdminPromptPreviewApi() {
  const apiBaseUrl = useApiBaseUrl();

  const loadPostWorkout = useCallback(
    async (userId: string, date: string) =>
      loadAdminPostWorkoutPromptPreview(apiBaseUrl, userId, date),
    [apiBaseUrl],
  );

  const loadCalendarCoach = useCallback(
    async (userId: string, date: string) =>
      loadAdminCalendarCoachPromptPreview(apiBaseUrl, userId, date),
    [apiBaseUrl],
  );

  const loadMesoCycle = useCallback(
    async (userId: string, date: string) =>
      loadAdminMesoCyclePromptPreview(apiBaseUrl, userId, date),
    [apiBaseUrl],
  );

  return useMemo(
    () => ({
      loadAdminPostWorkoutPromptPreview: loadPostWorkout,
      loadAdminCalendarCoachPromptPreview: loadCalendarCoach,
      loadAdminMesoCyclePromptPreview: loadMesoCycle,
    }),
    [loadCalendarCoach, loadMesoCycle, loadPostWorkout],
  );
}
