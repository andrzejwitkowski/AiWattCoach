import { AuthenticationError, get, HttpError, patch, post } from '../../../lib/httpClient';
import {
  saveWorkoutSummaryResponseSchema,
  sendMessageRequestSchema,
  sendMessageResponseSchema,
  updateRpeRequestSchema,
  workoutSummarySchema,
} from '../types';

const workoutTargetIdSchema = sendMessageRequestSchema.shape.content.transform((value) => value);

export type WorkoutSummaryDateRange = {
  oldest: string;
  newest: string;
};

function normalizeWorkoutTargetId(targetId: string): string {
  return workoutTargetIdSchema.parse(targetId);
}

function buildSummaryQuery(range?: WorkoutSummaryDateRange, view?: 'metadata'): string {
  const query = new URLSearchParams();
  if (range) {
    query.set('oldest', range.oldest);
    query.set('newest', range.newest);
  }
  if (view === 'metadata') {
    query.set('view', 'metadata');
  }
  const serialized = query.toString();
  return serialized ? `?${serialized}` : '';
}

export async function getWorkoutSummary(
  apiBaseUrl: string,
  targetId: string,
  range?: WorkoutSummaryDateRange,
) {
  const data = await get(
    apiBaseUrl,
    `/api/workout-summaries/${normalizeWorkoutTargetId(targetId)}${buildSummaryQuery(range)}`,
  );
  return workoutSummarySchema.parse(data);
}

export async function createWorkoutSummary(apiBaseUrl: string, targetId: string) {
  const data = await post<undefined, unknown>(
    apiBaseUrl,
    `/api/workout-summaries/${normalizeWorkoutTargetId(targetId)}`,
    undefined,
  );
  return workoutSummarySchema.parse(data);
}

export async function listWorkoutSummaries(
  apiBaseUrl: string,
  targetIds: string[],
  options?: {
    range?: WorkoutSummaryDateRange;
    view?: 'metadata';
  },
) {
  const normalizedIds = targetIds.map((targetId) => normalizeWorkoutTargetId(targetId));

  if (normalizedIds.length === 0) {
    return [];
  }

  const query = new URLSearchParams({ workoutIds: normalizedIds.join(',') });
  if (options?.range) {
    query.set('oldest', options.range.oldest);
    query.set('newest', options.range.newest);
  }
  if (options?.view === 'metadata') {
    query.set('view', 'metadata');
  }
  const data = await get(apiBaseUrl, `/api/workout-summaries?${query.toString()}`);
  return workoutSummarySchema.array().parse(data);
}

export async function updateWorkoutSummaryRpe(apiBaseUrl: string, workoutId: string, rpe: unknown) {
  const validated = updateRpeRequestSchema.parse({ rpe });
  const data = await patch<typeof validated, unknown>(
    apiBaseUrl,
    `/api/workout-summaries/${normalizeWorkoutTargetId(workoutId)}/rpe`,
    validated,
  );
  return workoutSummarySchema.parse(data);
}

export async function saveWorkoutSummary(apiBaseUrl: string, workoutId: string) {
  const data = await post<{ saved: boolean }, unknown>(
    apiBaseUrl,
    `/api/workout-summaries/${normalizeWorkoutTargetId(workoutId)}/state`,
    { saved: true },
  );
  return saveWorkoutSummaryResponseSchema.parse(data);
}

export async function reopenWorkoutSummary(apiBaseUrl: string, workoutId: string) {
  const data = await patch<{ saved: boolean }, unknown>(
    apiBaseUrl,
    `/api/workout-summaries/${normalizeWorkoutTargetId(workoutId)}/state`,
    { saved: false },
  );
  return workoutSummarySchema.parse(data);
}

export async function sendWorkoutSummaryMessage(apiBaseUrl: string, workoutId: string, payload: unknown) {
  const validated = sendMessageRequestSchema.parse(payload);
  const data = await post<typeof validated, unknown>(
    apiBaseUrl,
    `/api/workout-summaries/${normalizeWorkoutTargetId(workoutId)}/messages`,
    validated,
  );
  return sendMessageResponseSchema.parse(data);
}

export async function ensureWorkoutSummary(apiBaseUrl: string, workoutId: string) {
  try {
    return await getWorkoutSummary(apiBaseUrl, workoutId);
  } catch (error) {
    if (error instanceof AuthenticationError) {
      throw error;
    }

    if (error instanceof HttpError && error.status === 404) {
      return createWorkoutSummary(apiBaseUrl, workoutId);
    }

    throw error;
  }
}
