import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  generateMesoCyclePlan,
  loadMesoCycleCalendar,
  loadMesoCycleStatus,
} from './mesoCycle';

const originalFetch = global.fetch;

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('mesoCycle api', () => {
  it('parses meso cycle status', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({
        window: {
          mesoStart: '2026-06-01',
          mesoEnd: '2026-06-30',
          aiCoachLastDate: null,
        },
        hasPendingGeneration: false,
        latestOperation: null,
      }),
    });
    global.fetch = fetchMock as typeof fetch;

    const result = await loadMesoCycleStatus('');
    expect(result.window?.mesoStart).toBe('2026-06-01');
  });

  it('loads calendar with encoded query params', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => [
        {
          date: '2026-06-01',
          restDay: true,
          restDayReason: 'rest',
          name: 'Rest Day',
          rawWorkoutDoc: null,
          overlapStatus: 'active',
        },
      ],
    });
    global.fetch = fetchMock as typeof fetch;

    await loadMesoCycleCalendar('', '2026-06-01', '2026-06-30');
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/meso-cycle/calendar?from=2026-06-01&to=2026-06-30',
      expect.any(Object),
    );
  });

  it('surfaces backend error bodies on generate failures', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: false,
      status: 409,
      json: async () => ({ error: 'meso cycle generation is already pending' }),
    });
    global.fetch = fetchMock as typeof fetch;

    await expect(generateMesoCyclePlan('')).rejects.toThrow(
      'meso cycle generation is already pending',
    );
  });
});
