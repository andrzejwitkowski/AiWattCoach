import { describe, expect, it } from 'vitest';

import {
  createPlannedRestDay,
  deletePlannedRestDay,
  getPlannedRestDay,
  listPlannedRestDays,
  updatePlannedRestDay,
} from './plannedRestDays';
import { createFetchMock, useFetchMock } from '../../intervals/api/testHelpers';

const plannedRestDayFixture = {
  plannedRestDayId: 'prd-1',
  startDate: '2026-12-24',
  endDate: '2026-12-26',
  title: 'Holiday',
  note: 'Family trip',
  createdAtEpochSeconds: 1_700_000_000,
  updatedAtEpochSeconds: 1_700_000_000,
};

const updatedPlannedRestDayFixture = {
  ...plannedRestDayFixture,
  title: 'Winter break',
  note: 'No cycling',
  updatedAtEpochSeconds: 1_700_000_100,
};

describe('plannedRestDays api', () => {
  it('lists planned rest days for a date range', async () => {
    const fetchMock = useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(JSON.stringify([plannedRestDayFixture]), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      ),
    );

    const result = await listPlannedRestDays('', { oldest: '2026-12-01', newest: '2026-12-31' });

    expect(fetchMock).toHaveBeenCalledWith('/api/planned-rest-days?oldest=2026-12-01&newest=2026-12-31', {
      method: 'GET',
      headers: {
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: undefined,
    });
    expect(result).toEqual([plannedRestDayFixture]);
  });

  it('creates, loads, updates, and deletes planned rest days', async () => {
    const fetchMock = useFetchMock(
      createFetchMock()
        .mockResolvedValueOnce(
          new Response(JSON.stringify(plannedRestDayFixture), {
            status: 201,
            headers: { 'content-type': 'application/json' },
          }),
        )
        .mockResolvedValueOnce(
          new Response(JSON.stringify(plannedRestDayFixture), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          }),
        )
        .mockResolvedValueOnce(
          new Response(JSON.stringify(updatedPlannedRestDayFixture), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          }),
        )
        .mockResolvedValueOnce(new Response(null, { status: 204 })),
    );

    const created = await createPlannedRestDay('', {
      startDate: '2026-12-24',
      endDate: '2026-12-26',
      title: 'Holiday',
      note: 'Family trip',
    });
    const loaded = await getPlannedRestDay('', 'prd-1');
    const updated = await updatePlannedRestDay('', 'prd-1', {
      startDate: '2026-12-24',
      endDate: '2026-12-26',
      title: 'Winter break',
      note: 'No cycling',
    });
    await deletePlannedRestDay('', 'prd-1');

    expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/planned-rest-days', {
      method: 'POST',
      headers: expect.objectContaining({
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      }),
      credentials: 'include',
      body: JSON.stringify({
        startDate: '2026-12-24',
        endDate: '2026-12-26',
        title: 'Holiday',
        note: 'Family trip',
      }),
    });
    expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/planned-rest-days/prd-1', {
      method: 'GET',
      headers: expect.objectContaining({
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      }),
      credentials: 'include',
      body: undefined,
    });
    expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/planned-rest-days/prd-1', {
      method: 'PUT',
      headers: expect.objectContaining({
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      }),
      credentials: 'include',
      body: JSON.stringify({
        startDate: '2026-12-24',
        endDate: '2026-12-26',
        title: 'Winter break',
        note: 'No cycling',
      }),
    });
    expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/planned-rest-days/prd-1', {
      method: 'DELETE',
      headers: expect.objectContaining({
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      }),
      credentials: 'include',
      body: undefined,
    });
    expect(created).toEqual(plannedRestDayFixture);
    expect(loaded).toEqual(plannedRestDayFixture);
    expect(updated).toEqual(updatedPlannedRestDayFixture);
  });

  it('rejects malformed planned rest day payloads at the zod boundary', async () => {
    useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            plannedRestDayId: 'prd-1',
            startDate: '2026-12-24',
            endDate: '2026-12-26',
            title: 'Holiday',
            note: 'Family trip',
            createdAtEpochSeconds: 'not-a-number',
            updatedAtEpochSeconds: 1_700_000_000,
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    await expect(getPlannedRestDay('', 'prd-1')).rejects.toThrow();
  });
});
