import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  makeActivity,
  makeActivityInterval,
  makeActivityStream,
  makeSelection,
} from '../testData';
import { mockedLoadActivity, mockedLoadEvent, setChartRect } from './WorkoutDetailModal.testHelpers';
import { WorkoutDetailModal } from './WorkoutDetailModal';

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function renderActivityModal(activity = makeActivity()) {
  return render(
    <WorkoutDetailModal
      apiBaseUrl=""
      selection={makeSelection({ activity })}
      onClose={vi.fn()}
    />,
  );
}

describe('WorkoutDetailModal interval interaction', () => {
  it('highlights the matching interval chip and row while hovering the chart', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a47',
        startDateLocal: '2026-04-02T08:00:00',
        name: 'Highlight Ride',
        distanceMeters: 30000,
        movingTimeSeconds: 1200,
        elapsedTimeSeconds: 1200,
        hasHeartRate: true,
        streamTypes: ['watts'],
        metrics: { trainingStressScore: 30, normalizedPowerWatts: 220, intensityFactor: 0.8, averagePowerWatts: 210, ftpWatts: 280 },
        details: {
          intervals: [
            makeActivityInterval({ id: 1, label: 'Ride 1', startIndex: 0, endIndex: 599, averagePowerWatts: 200, averageHeartRateBpm: 140, zone: 3 }),
            makeActivityInterval({ id: 2, label: 'Ride 2', startIndex: 600, endIndex: 1199, startTimeSeconds: 600, endTimeSeconds: 1200, averagePowerWatts: 240, averageHeartRateBpm: 150, zone: 4 }),
          ],
          streams: [makeActivityStream({ data: Array.from({ length: 1200 }, (_, index) => (index < 600 ? 200 : 240)) })],
        },
      }),
    );

    renderActivityModal(
      makeActivity({
        id: 'a47',
        startDateLocal: '2026-04-02T08:00:00',
        name: 'Highlight Ride',
        movingTimeSeconds: 1200,
        elapsedTimeSeconds: 1200,
        hasHeartRate: true,
      }),
    );

    await waitFor(() => expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument());

    const chart = screen.getByLabelText(/power chart/i);
    setChartRect(chart);
    fireEvent.mouseMove(chart, { clientX: 750, clientY: 80 });

    const activeChip = screen.getAllByText('Ride 2').find((element) => element.getAttribute('data-interval-chip-active') === 'true');
    expect(activeChip).toBeTruthy();
    expect(document.querySelector('[data-interval-row-active="true"]')).toHaveTextContent('Ride 2');
  });

  it('does not auto-scroll the interval list when chart hover changes the active ride', async () => {
    const scrollIntoView = vi.fn();
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    try {
      mockedLoadEvent.mockResolvedValue(undefined as never);
      mockedLoadActivity.mockResolvedValue(
        makeActivity({
          id: 'a47-scroll',
          startDateLocal: '2026-04-02T08:00:00',
          name: 'No Scroll Ride',
          distanceMeters: 30000,
          movingTimeSeconds: 1200,
          elapsedTimeSeconds: 1200,
          hasHeartRate: true,
          streamTypes: ['watts'],
          metrics: { trainingStressScore: 30, normalizedPowerWatts: 220, intensityFactor: 0.8, averagePowerWatts: 210, ftpWatts: 280 },
          details: {
            intervals: [
              makeActivityInterval({ id: 1, label: 'Ride 1', startIndex: 0, endIndex: 599, averagePowerWatts: 200, averageHeartRateBpm: 140, zone: 3 }),
              makeActivityInterval({ id: 2, label: 'Ride 2', startIndex: 600, endIndex: 1199, startTimeSeconds: 600, endTimeSeconds: 1200, averagePowerWatts: 240, averageHeartRateBpm: 150, zone: 4 }),
            ],
            streams: [makeActivityStream({ data: Array.from({ length: 1200 }, (_, index) => (index < 600 ? 200 : 240)) })],
          },
        }),
      );

      renderActivityModal(
        makeActivity({
          id: 'a47-scroll',
          startDateLocal: '2026-04-02T08:00:00',
          name: 'No Scroll Ride',
          movingTimeSeconds: 1200,
          elapsedTimeSeconds: 1200,
          hasHeartRate: true,
        }),
      );

      await waitFor(() => expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument());

      const chart = screen.getByLabelText(/power chart/i);
      setChartRect(chart);
      fireEvent.mouseMove(chart, { clientX: 750, clientY: 80 });

      expect(document.querySelector('[data-interval-row-active="true"]')).toHaveTextContent('Ride 2');
      expect(scrollIntoView).not.toHaveBeenCalled();
    } finally {
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('lets interval chips and rows drive the chart highlight in reverse', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a48',
        startDateLocal: '2026-04-03T08:00:00',
        name: 'Reverse Highlight Ride',
        distanceMeters: 30000,
        movingTimeSeconds: 1200,
        elapsedTimeSeconds: 1200,
        hasHeartRate: true,
        streamTypes: ['watts'],
        metrics: { trainingStressScore: 30, normalizedPowerWatts: 220, intensityFactor: 0.8, averagePowerWatts: 210, ftpWatts: 280 },
        details: {
          intervals: [
            makeActivityInterval({ id: 1, label: 'Ride 1', startIndex: 0, endIndex: 599, averagePowerWatts: 200, averageHeartRateBpm: 140, zone: 3 }),
            makeActivityInterval({ id: 2, label: 'Ride 2', startIndex: 600, endIndex: 1199, startTimeSeconds: 600, endTimeSeconds: 1200, averagePowerWatts: 240, averageHeartRateBpm: 150, zone: 4 }),
          ],
          streams: [makeActivityStream({ data: Array.from({ length: 1200 }, (_, index) => (index < 600 ? 200 : 240)) })],
        },
      }),
    );

    renderActivityModal(
      makeActivity({
        id: 'a48',
        startDateLocal: '2026-04-03T08:00:00',
        name: 'Reverse Highlight Ride',
        movingTimeSeconds: 1200,
        elapsedTimeSeconds: 1200,
        hasHeartRate: true,
      }),
    );

    await waitFor(() => expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument());

    const ride2Chip = screen.getAllByText('Ride 2').find((element) => element.getAttribute('data-interval-chip-active') === 'false');
    expect(ride2Chip).toBeTruthy();
    fireEvent.mouseEnter(ride2Chip as Element);

    expect(screen.getAllByText('Ride 2').some((element) => element.getAttribute('data-interval-chip-active') === 'true')).toBe(true);
    expect(document.querySelector('[data-interval-row-active="true"]')).toHaveTextContent('Ride 2');
    expect(document.querySelector('[data-hover-power-readout="true"]')).toHaveTextContent('240 W');

    const ride1Row = Array.from(document.querySelectorAll('[data-interval-row-active]')).find((element) => element.textContent?.includes('Ride 1')) as HTMLElement;
    fireEvent.click(ride1Row);

    expect(document.querySelectorAll('[data-interval-row-active="true"]').length).toBeGreaterThan(0);
    fireEvent.mouseEnter(ride1Row);
    expect(document.querySelector('[data-hover-power-readout="true"]')).toHaveTextContent('200 W');
  });

  it('keeps chart overlays aligned when hidden intervals precede visible ones', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a49',
        startDateLocal: '2026-04-04T08:00:00',
        name: 'Hidden Interval Ride',
        distanceMeters: 20000,
        movingTimeSeconds: 1200,
        elapsedTimeSeconds: 1200,
        hasHeartRate: true,
        streamTypes: ['watts'],
        metrics: { trainingStressScore: 20, normalizedPowerWatts: 210, intensityFactor: 0.75, averagePowerWatts: 205, ftpWatts: 280 },
        details: {
          intervals: [
            makeActivityInterval({ id: 91, label: null, intervalType: null, startIndex: null, endIndex: null, startTimeSeconds: 0, endTimeSeconds: 300, movingTimeSeconds: 300, elapsedTimeSeconds: 300, zone: null }),
            makeActivityInterval({ id: 92, label: 'Ride 1', intervalType: 'WORK', startIndex: null, endIndex: null, startTimeSeconds: null, endTimeSeconds: null, movingTimeSeconds: 600, elapsedTimeSeconds: 600, averagePowerWatts: 220, averageHeartRateBpm: 145, zone: 3 }),
          ],
          streams: [makeActivityStream({ data: Array.from({ length: 1200 }, (_, index) => (index < 300 ? 120 : 220)) })],
        },
      }),
    );

    renderActivityModal(
      makeActivity({
        id: 'a49',
        startDateLocal: '2026-04-04T08:00:00',
        name: 'Hidden Interval Ride',
        movingTimeSeconds: 1200,
        elapsedTimeSeconds: 1200,
        hasHeartRate: true,
      }),
    );

    await waitFor(() => expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument());

    const chart = screen.getByLabelText(/power chart/i);
    setChartRect(chart);
    fireEvent.mouseMove(chart, { clientX: 400, clientY: 80 });

    expect(document.querySelector('[data-interval-row-active="true"]')).toHaveTextContent('Ride 1');
    expect(document.querySelector('[data-hover-power-readout="true"]')).toHaveTextContent('220 W');
  });

  it('supports keyboard activation for interval rows and chips', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a50',
        startDateLocal: '2026-04-05T08:00:00',
        name: 'Keyboard Ride',
        distanceMeters: 30000,
        movingTimeSeconds: 1200,
        elapsedTimeSeconds: 1200,
        hasHeartRate: true,
        streamTypes: ['watts'],
        metrics: { trainingStressScore: 20, normalizedPowerWatts: 210, intensityFactor: 0.75, averagePowerWatts: 205, ftpWatts: 280 },
        details: {
          intervals: [
            makeActivityInterval({ id: 1, label: 'Ride 1', averagePowerWatts: 200, averageHeartRateBpm: 140, zone: 3 }),
            makeActivityInterval({ id: 2, label: 'Ride 2', startTimeSeconds: 600, endTimeSeconds: 1200, averagePowerWatts: 240, averageHeartRateBpm: 150, zone: 4 }),
          ],
          streams: [makeActivityStream({ data: Array.from({ length: 1200 }, (_, index) => (index < 600 ? 200 : 240)) })],
        },
      }),
    );

    renderActivityModal(
      makeActivity({
        id: 'a50',
        startDateLocal: '2026-04-05T08:00:00',
        name: 'Keyboard Ride',
        movingTimeSeconds: 1200,
        elapsedTimeSeconds: 1200,
        hasHeartRate: true,
      }),
    );

    await waitFor(() => expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument());

    const ride2Chip = screen.getAllByText('Ride 2').find((element) => element.getAttribute('data-interval-chip-active') === 'false') as HTMLElement;
    ride2Chip.focus();
    fireEvent.keyDown(ride2Chip, { key: 'Enter' });

    expect(document.querySelector('[data-interval-row-active="true"]')).toHaveTextContent('Ride 2');

    const ride1Row = Array.from(document.querySelectorAll('[data-interval-row-active]')).find((element) => element.textContent?.includes('Ride 1')) as HTMLElement;
    ride1Row.focus();
    fireEvent.keyDown(ride1Row, { key: ' ' });

    const activeRows = Array.from(document.querySelectorAll('[data-interval-row-active="true"]'));
    expect(activeRows.some((element) => element.textContent?.includes('Ride 1'))).toBe(true);
  });
});
