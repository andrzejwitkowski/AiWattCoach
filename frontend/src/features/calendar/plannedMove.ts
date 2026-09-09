import { z } from 'zod';

export type PlannedWorkoutMoveInput = {
  plannedWorkoutId: string;
  fromDate: string;
  toDate: string;
};

export type PlannedMoveDragPayload = {
  plannedWorkoutId: string;
  fromDate: string;
};

export const PLANNED_MOVE_MIME = 'application/x-aiwattcoach-planned-move';

const plannedMoveDragPayloadSchema = z.object({
  plannedWorkoutId: z.string().min(1),
  fromDate: z.string().min(1),
});

export function plannedMoveWorkoutId(event: {
  projectedWorkout?: { projectedWorkoutId: string } | null;
  calendarEntryId?: string | null;
}): string | null {
  return event.projectedWorkout?.projectedWorkoutId ?? event.calendarEntryId ?? null;
}

export function parsePlannedMovePayload(raw: string): PlannedMoveDragPayload | null {
  try {
    return plannedMoveDragPayloadSchema.parse(JSON.parse(raw));
  } catch {
    return null;
  }
}
