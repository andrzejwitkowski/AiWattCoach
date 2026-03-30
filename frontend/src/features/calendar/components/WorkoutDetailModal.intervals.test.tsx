import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  makeActivity,
  makeActivityInterval,
  makeActivityStream,
  makeSelection,
} from '../testData';
import { mockedLoadActivity, mockedLoadEvent } from './WorkoutDetailModal.testHelpers';
import { WorkoutDetailModal } from './WorkoutDetailModal';

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('WorkoutDetailModal interval sections', () => {
  it('renders completed-only interval sections from enriched activity details', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a25',
        startDateLocal: '2026-03-28T08:00:00',
        name: 'Outside Tempo',
        distanceMeters: 32000,
        movingTimeSeconds: 3600,
        elapsedTimeSeconds: 3660,
        averageSpeedMps: 8.9,
        averageHeartRateBpm: 151,
        averageCadenceRpm: 88,
        hasHeartRate: true,
        streamTypes: ['watts', 'heartrate'],
        metrics: { trainingStressScore: 74, normalizedPowerWatts: 249, intensityFactor: 0.89, averagePowerWatts: 236, ftpWatts: 280 },
        details: {
          intervals: [
            makeActivityInterval({ id: 1, label: 'Tempo Block 1', groupId: 'tempo', startIndex: 0, endIndex: 599, startTimeSeconds: 0, endTimeSeconds: 600, movingTimeSeconds: 600, elapsedTimeSeconds: 600, distanceMeters: 5000, averagePowerWatts: 245, normalizedPowerWatts: 252, trainingStressScore: 12.4, averageHeartRateBpm: 148, averageCadenceRpm: 87, averageSpeedMps: 8.3, zone: 3 }),
            makeActivityInterval({ id: 2, label: 'Tempo Block 2', groupId: 'tempo', startIndex: 900, endIndex: 1499, startTimeSeconds: 900, endTimeSeconds: 1500, movingTimeSeconds: 600, elapsedTimeSeconds: 600, distanceMeters: 5100, averagePowerWatts: 255, normalizedPowerWatts: 261, trainingStressScore: 13.2, averageHeartRateBpm: 154, averageCadenceRpm: 89, averageSpeedMps: 8.5, zone: 4 }),
          ],
          streams: [
            makeActivityStream({ data: [210, 240, 255, 260] }),
            makeActivityStream({ streamType: 'heartrate', name: 'Heart Rate', data: [138, 146, 152, 156] }),
          ],
        },
      }),
    );

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-28',
          activity: makeActivity({ id: 'a25', startDateLocal: '2026-03-28T08:00:00', name: 'Outside Tempo', movingTimeSeconds: 3600, elapsedTimeSeconds: 3660, hasHeartRate: true }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText(/completed workout/i)).toBeInTheDocument());

    expect(screen.getByText('Outside Tempo')).toBeInTheDocument();
    expect(screen.getByText('249 W')).toBeInTheDocument();
    expect(screen.getByText('74 TSS')).toBeInTheDocument();
    expect(screen.getByText(/completed intervals/i)).toBeInTheDocument();
    const fills = Array.from(document.querySelectorAll('[data-interval-duration-fill="true"]')) as HTMLDivElement[];
    expect(fills.length).toBeGreaterThanOrEqual(2);
    expect(fills[0].style.width).toBe('16.666666666666664%');
    expect(fills[1].style.width).toBe('16.666666666666664%');
    expect(screen.getAllByText('Tempo Block 1').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Tempo Block 2').length).toBeGreaterThan(0);
    expect(screen.getByText('245 W')).toBeInTheDocument();
    expect(screen.getByText('255 W')).toBeInTheDocument();
    expect(screen.getAllByText('10m')).toHaveLength(2);
  });

  it('excludes metadata-only completed intervals from the rendered section', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a26',
        startDateLocal: '2026-03-29T08:00:00',
        name: 'Metadata Filter Ride',
        distanceMeters: 28000,
        movingTimeSeconds: 2400,
        elapsedTimeSeconds: 2460,
        averageSpeedMps: 8.2,
        averageHeartRateBpm: 149,
        averageCadenceRpm: 87,
        hasHeartRate: true,
        streamTypes: ['watts'],
        metrics: { trainingStressScore: 46, normalizedPowerWatts: 226, intensityFactor: 0.81, averagePowerWatts: 214, ftpWatts: 280 },
        details: {
          intervals: [
            makeActivityInterval({ id: 91, label: null, groupId: 'meta', startIndex: 0, endIndex: 599, startTimeSeconds: 0, endTimeSeconds: 600, movingTimeSeconds: null, elapsedTimeSeconds: null, distanceMeters: 4200, averagePowerWatts: null, normalizedPowerWatts: 230, trainingStressScore: 9.2, averageCadenceRpm: 88, averageSpeedMps: 8.1, zone: 3 }),
            makeActivityInterval({ id: 92, label: 'Shown Interval', groupId: 'meta', startIndex: 600, endIndex: 1199, startTimeSeconds: 600, endTimeSeconds: 1200, movingTimeSeconds: null, elapsedTimeSeconds: 600, distanceMeters: null, averagePowerWatts: null, normalizedPowerWatts: null, trainingStressScore: null, averageCadenceRpm: null, averageSpeedMps: null, zone: null }),
          ],
          streams: [makeActivityStream({ data: [180, 220, 230] })],
        },
      }),
    );

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-29',
          activity: makeActivity({ id: 'a26', startDateLocal: '2026-03-29T08:00:00', name: 'Metadata Filter Ride', movingTimeSeconds: 2400, elapsedTimeSeconds: 2460, hasHeartRate: true }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText(/completed workout/i)).toBeInTheDocument());

    expect(screen.getByText(/completed intervals/i)).toBeInTheDocument();
    expect(screen.getAllByText('Shown Interval').length).toBeGreaterThan(0);
    expect(screen.queryByText('Ride 1')).not.toBeInTheDocument();
    const fills = Array.from(document.querySelectorAll('[data-interval-duration-fill="true"]')) as HTMLDivElement[];
    expect(fills.length).toBeGreaterThanOrEqual(1);
    expect(fills[fills.length - 1].style.width).toBe('25%');
    expect(screen.getAllByText('10m')).toHaveLength(1);
  });
});
