import { AuthenticationError, get, HttpError, patch, post } from '../../../lib/httpClient';
import {
  sendMessageRequestSchema,
  sendMessageResponseSchema,
  updateRpeRequestSchema,
  workoutSummarySchema,
} from '../types';

const eventIdSchema = sendMessageRequestSchema.shape.content.transform((value) => value);

function normalizeEventId(eventId: string): string {
  return eventIdSchema.parse(eventId);
}

export async function getWorkoutSummary(apiBaseUrl: string, eventId: string) {
  const data = await get(apiBaseUrl, `/api/workout-summaries/${normalizeEventId(eventId)}`);
  return workoutSummarySchema.parse(data);
}

export async function createWorkoutSummary(apiBaseUrl: string, eventId: string) {
  const data = await post<undefined, unknown>(
    apiBaseUrl,
    `/api/workout-summaries/${normalizeEventId(eventId)}`,
    undefined,
  );
  return workoutSummarySchema.parse(data);
}

export async function listWorkoutSummaries(apiBaseUrl: string, eventIds: string[]) {
  const normalizedIds = eventIds.map((eventId) => normalizeEventId(eventId));

  if (normalizedIds.length === 0) {
    return [];
  }

  const query = new URLSearchParams({ eventIds: normalizedIds.join(',') });
  const data = await get(apiBaseUrl, `/api/workout-summaries?${query.toString()}`);
  return workoutSummarySchema.array().parse(data);
}

export async function updateWorkoutSummaryRpe(apiBaseUrl: string, eventId: string, rpe: unknown) {
  const validated = updateRpeRequestSchema.parse({ rpe });
  const data = await patch<typeof validated, unknown>(
    apiBaseUrl,
    `/api/workout-summaries/${normalizeEventId(eventId)}/rpe`,
    validated,
  );
  return workoutSummarySchema.parse(data);
}

export async function sendWorkoutSummaryMessage(apiBaseUrl: string, eventId: string, payload: unknown) {
  const validated = sendMessageRequestSchema.parse(payload);
  const data = await post<typeof validated, unknown>(
    apiBaseUrl,
    `/api/workout-summaries/${normalizeEventId(eventId)}/messages`,
    validated,
  );
  return sendMessageResponseSchema.parse(data);
}

export async function ensureWorkoutSummary(apiBaseUrl: string, eventId: string) {
  try {
    return await getWorkoutSummary(apiBaseUrl, eventId);
  } catch (error) {
    if (error instanceof AuthenticationError) {
      throw error;
    }

    if (error instanceof HttpError && error.status === 404) {
      return createWorkoutSummary(apiBaseUrl, eventId);
    }

    throw error;
  }
}
