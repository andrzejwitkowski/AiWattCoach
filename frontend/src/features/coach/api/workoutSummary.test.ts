import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  createWorkoutSummary,
  getWorkoutSummary,
  listWorkoutSummaries,
  reopenWorkoutSummary,
  saveWorkoutSummary,
  updateWorkoutSummaryRpe,
} from './workoutSummary';

const originalFetch = global.fetch;

const summaryFixture = {
  id: 'summary-1',
  workoutId: '101',
  rpe: 7,
  messages: [
    {
      id: 'message-1',
      role: 'coach',
      content: 'How did that final interval feel?',
      createdAtEpochSeconds: 1711000100,
    },
  ],
  savedAtEpochSeconds: null,
  createdAtEpochSeconds: 1711000000,
  updatedAtEpochSeconds: 1711000200,
};

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('workoutSummary api', () => {
  it('loads a workout summary', async () => {
    global.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(summaryFixture), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    ) as typeof fetch;

    const result = await getWorkoutSummary('', '101');

    expect(result.workoutId).toBe('101');
    expect(global.fetch).toHaveBeenCalledWith('/api/workout-summaries/101', expect.any(Object));
  });

  it('creates a workout summary', async () => {
    global.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(summaryFixture), {
        status: 201,
        headers: { 'content-type': 'application/json' },
      }),
    ) as typeof fetch;

    const result = await createWorkoutSummary('', '101');

    expect(result.id).toBe('summary-1');
    expect(global.fetch).toHaveBeenCalledWith('/api/workout-summaries/101', expect.objectContaining({ method: 'POST' }));
  });

  it('lists workout summaries by workout id', async () => {
    global.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify([summaryFixture]), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    ) as typeof fetch;

    const result = await listWorkoutSummaries('', ['101', '102']);

    expect(result).toHaveLength(1);
    expect(global.fetch).toHaveBeenCalledWith(
      '/api/workout-summaries?workoutIds=101%2C102',
      expect.any(Object),
    );
  });

  it('updates workout summary rpe', async () => {
    global.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ ...summaryFixture, rpe: 8 }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    ) as typeof fetch;

    const result = await updateWorkoutSummaryRpe('', '101', 8);

    expect(result.rpe).toBe(8);
    expect(global.fetch).toHaveBeenCalledWith(
      '/api/workout-summaries/101/rpe',
      expect.objectContaining({
        method: 'PATCH',
        body: JSON.stringify({ rpe: 8 }),
      }),
    );
  });

  it('saves workout summary', async () => {
    global.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ summary: { ...summaryFixture, savedAtEpochSeconds: 1711000300 } }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    ) as typeof fetch;

    const result = await saveWorkoutSummary('', '101');

    expect(result.savedAtEpochSeconds).toBe(1711000300);
    expect(global.fetch).toHaveBeenCalledWith(
      '/api/workout-summaries/101/state',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ saved: true }),
      }),
    );
  });

  it('reopens workout summary', async () => {
    global.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ summary: summaryFixture }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    ) as typeof fetch;

    const result = await reopenWorkoutSummary('', '101');

    expect(result.savedAtEpochSeconds).toBeNull();
    expect(global.fetch).toHaveBeenCalledWith(
      '/api/workout-summaries/101/state',
      expect.objectContaining({
        method: 'PATCH',
        body: JSON.stringify({ saved: false }),
      }),
    );
  });
});
