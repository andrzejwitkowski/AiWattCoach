import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import type { CalendarWeek } from '../types';
import { CalendarWeekSection } from './CalendarWeekSection';

function emptyEventDefinition() {
  return {
    rawWorkoutDoc: null,
    intervals: [],
    segments: [],
    summary: {
      totalSegments: 0,
      totalDurationSeconds: 0,
      estimatedNormalizedPowerWatts: null,
      estimatedAveragePowerWatts: null,
      estimatedIntensityFactor: null,
      estimatedTrainingStressScore: null,
    },
  };
}

function createWeek(status: CalendarWeek['status']): CalendarWeek {
  return {
    weekNumber: 12,
    weekKey: '2026-03-23',
    mondayDate: new Date(2026, 2, 23),
    days: Array.from({ length: 7 }, (_, index) => ({
      date: new Date(2026, 2, 23 + index),
      dateKey: `2026-03-${String(23 + index).padStart(2, '0')}`,
      events: [],
      activities: [],
      labels: [],
    })),
    summary: {
      totalTss: 0,
      targetTss: null,
      totalCalories: 0,
      totalDurationSeconds: 0,
      targetDurationSeconds: null,
      totalDistanceMeters: 0,
    },
    status,
  };
}

describe('CalendarWeekSection', () => {
  it('renders a quiet placeholder for idle weeks', () => {
    render(<CalendarWeekSection week={createWeek('idle')} />);

    expect(screen.queryByText(/fetching data/i)).not.toBeInTheDocument();
  });

  it('renders spinner label for loading weeks', () => {
    render(<CalendarWeekSection week={createWeek('loading')} />);

    expect(screen.getByText(/fetching data/i)).toBeInTheDocument();
  });

  it('keeps the planned workout attached when the day activity is unrelated to it', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [{
        id: 11,
        startDateLocal: '2026-03-23',
        name: 'Planned workout',
        category: 'WORKOUT',
        description: null,
        indoor: false,
        color: null,
        eventDefinition: {
          rawWorkoutDoc: '- 20min 95%',
          intervals: [],
          segments: [],
          summary: {
            totalSegments: 0,
            totalDurationSeconds: 0,
            estimatedNormalizedPowerWatts: null,
            estimatedAveragePowerWatts: null,
            estimatedIntensityFactor: null,
            estimatedTrainingStressScore: null,
          },
        },
        actualWorkout: null,
      }],
      activities: [{
        id: 'a-unrelated',
        startDateLocal: '2026-03-23T08:00:00',
        startDate: '2026-03-23T07:00:00Z',
        name: 'Unrelated ride',
        description: null,
        activityType: 'Ride',
        source: null,
        externalId: null,
        deviceName: null,
        distanceMeters: null,
        movingTimeSeconds: null,
        elapsedTimeSeconds: null,
        totalElevationGainMeters: null,
        averageSpeedMps: null,
        averageHeartRateBpm: null,
        averageCadenceRpm: null,
        trainer: false,
        commute: false,
        race: false,
        hasHeartRate: false,
        streamTypes: [],
        tags: [],
        metrics: {
          trainingStressScore: null,
          normalizedPowerWatts: null,
          intensityFactor: null,
          efficiencyFactor: null,
          variabilityIndex: null,
          averagePowerWatts: null,
          ftpWatts: null,
          totalWorkJoules: null,
          calories: null,
          trimp: null,
          powerLoad: null,
          heartRateLoad: null,
          paceLoad: null,
          strainScore: null,
        },
        details: {
          intervals: [],
          intervalGroups: [],
          streams: [],
          intervalSummary: [],
          skylineChart: [],
          powerZoneTimes: [],
          heartRateZoneTimes: [],
          paceZoneTimes: [],
          gapZoneTimes: [],
        },
      }],
    };

    const onSelectWorkout = vi.fn();
    render(<CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} />);

    await userEvent.click(screen.getByRole('button', { name: /unrelated ride/i }));

    expect(onSelectWorkout).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      event: week.days[0].events[0],
      activity: week.days[0].activities[0],
    });
  });

  it('keeps planned-only days selectable as planned workouts', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [{
        id: 99,
        startDateLocal: '2026-03-23',
        name: 'Coach Build',
        category: 'WORKOUT',
        description: null,
        indoor: false,
        color: null,
        eventDefinition: {
          rawWorkoutDoc: '- 45min 80%',
          intervals: [],
          segments: [],
          summary: {
            totalSegments: 1,
            totalDurationSeconds: 2700,
            estimatedNormalizedPowerWatts: null,
            estimatedAveragePowerWatts: null,
            estimatedIntensityFactor: 0.8,
            estimatedTrainingStressScore: 42,
          },
        },
        actualWorkout: null,
      }],
      activities: [],
    };

    const onSelectWorkout = vi.fn();
    render(<CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} />);

    await userEvent.click(screen.getByRole('button', { name: /coach build/i }));

    expect(onSelectWorkout).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      event: week.days[0].events[0],
      activity: null,
    });
  });

  it('selects the matched activity when the event includes actual workout data', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [{
        id: 12,
        startDateLocal: '2026-03-23',
        name: 'Completed workout',
        category: 'WORKOUT',
        description: null,
        indoor: false,
        color: null,
        eventDefinition: {
          rawWorkoutDoc: '- 20min 95%',
          intervals: [],
          segments: [],
          summary: {
            totalSegments: 0,
            totalDurationSeconds: 0,
            estimatedNormalizedPowerWatts: null,
            estimatedAveragePowerWatts: null,
            estimatedIntensityFactor: null,
            estimatedTrainingStressScore: null,
          },
        },
        actualWorkout: {
          activityId: 'a-match',
          activityName: 'Matched ride',
          startDateLocal: '2026-03-23T08:00:00',
          powerValues: [],
          cadenceValues: [],
          heartRateValues: [],
          speedValues: [],
          averagePowerWatts: null,
          normalizedPowerWatts: null,
          trainingStressScore: null,
          intensityFactor: null,
          complianceScore: 0.8,
          matchedIntervals: [],
        },
      }],
      activities: [{
        id: 'a-match',
        startDateLocal: '2026-03-23T08:00:00',
        startDate: '2026-03-23T07:00:00Z',
        name: 'Matched ride',
        description: null,
        activityType: 'Ride',
        source: null,
        externalId: null,
        deviceName: null,
        distanceMeters: null,
        movingTimeSeconds: null,
        elapsedTimeSeconds: null,
        totalElevationGainMeters: null,
        averageSpeedMps: null,
        averageHeartRateBpm: null,
        averageCadenceRpm: null,
        trainer: false,
        commute: false,
        race: false,
        hasHeartRate: false,
        streamTypes: [],
        tags: [],
        metrics: {
          trainingStressScore: null,
          normalizedPowerWatts: null,
          intensityFactor: null,
          efficiencyFactor: null,
          variabilityIndex: null,
          averagePowerWatts: null,
          ftpWatts: null,
          totalWorkJoules: null,
          calories: null,
          trimp: null,
          powerLoad: null,
          heartRateLoad: null,
          paceLoad: null,
          strainScore: null,
        },
        details: {
          intervals: [],
          intervalGroups: [],
          streams: [],
          intervalSummary: [],
          skylineChart: [],
          powerZoneTimes: [],
          heartRateZoneTimes: [],
          paceZoneTimes: [],
          gapZoneTimes: [],
        },
      }],
    };

    const onSelectWorkout = vi.fn();
    render(<CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} />);

    await userEvent.click(screen.getByRole('button', { name: /matched ride/i }));

    expect(onSelectWorkout).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      event: week.days[0].events[0],
      activity: week.days[0].activities[0],
    });
  });

  it('selects the matched activity even when it is not the first activity of the day', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [{
        id: 13,
        startDateLocal: '2026-03-23',
        name: 'Completed workout',
        category: 'WORKOUT',
        description: null,
        indoor: false,
        color: null,
        eventDefinition: {
          rawWorkoutDoc: '- 20min 95%',
          intervals: [],
          segments: [],
          summary: {
            totalSegments: 0,
            totalDurationSeconds: 0,
            estimatedNormalizedPowerWatts: null,
            estimatedAveragePowerWatts: null,
            estimatedIntensityFactor: null,
            estimatedTrainingStressScore: null,
          },
        },
        actualWorkout: {
          activityId: 'a-match',
          activityName: 'Matched ride',
          startDateLocal: '2026-03-23T08:30:00',
          powerValues: [],
          cadenceValues: [],
          heartRateValues: [],
          speedValues: [],
          averagePowerWatts: null,
          normalizedPowerWatts: null,
          trainingStressScore: null,
          intensityFactor: null,
          complianceScore: 0.8,
          matchedIntervals: [],
        },
      }],
      activities: [
        {
          id: 'a-other',
          startDateLocal: '2026-03-23T07:00:00',
          startDate: '2026-03-23T06:00:00Z',
          name: 'Morning spin',
          description: null,
          activityType: 'Ride',
          source: null,
          externalId: null,
          deviceName: null,
          distanceMeters: null,
          movingTimeSeconds: null,
          elapsedTimeSeconds: null,
          totalElevationGainMeters: null,
          averageSpeedMps: null,
          averageHeartRateBpm: null,
          averageCadenceRpm: null,
          trainer: false,
          commute: false,
          race: false,
          hasHeartRate: false,
          streamTypes: [],
          tags: [],
          metrics: {
            trainingStressScore: null,
            normalizedPowerWatts: null,
            intensityFactor: null,
            efficiencyFactor: null,
            variabilityIndex: null,
            averagePowerWatts: null,
            ftpWatts: null,
            totalWorkJoules: null,
            calories: null,
            trimp: null,
            powerLoad: null,
            heartRateLoad: null,
            paceLoad: null,
            strainScore: null,
          },
          details: {
            intervals: [],
            intervalGroups: [],
            streams: [],
            intervalSummary: [],
            skylineChart: [],
            powerZoneTimes: [],
            heartRateZoneTimes: [],
            paceZoneTimes: [],
            gapZoneTimes: [],
          },
        },
        {
          id: 'a-match',
          startDateLocal: '2026-03-23T08:30:00',
          startDate: '2026-03-23T07:30:00Z',
          name: 'Matched ride',
          description: null,
          activityType: 'Ride',
          source: null,
          externalId: null,
          deviceName: null,
          distanceMeters: null,
          movingTimeSeconds: null,
          elapsedTimeSeconds: null,
          totalElevationGainMeters: null,
          averageSpeedMps: null,
          averageHeartRateBpm: null,
          averageCadenceRpm: null,
          trainer: false,
          commute: false,
          race: false,
          hasHeartRate: false,
          streamTypes: [],
          tags: [],
          metrics: {
            trainingStressScore: null,
            normalizedPowerWatts: null,
            intensityFactor: null,
            efficiencyFactor: null,
            variabilityIndex: null,
            averagePowerWatts: null,
            ftpWatts: null,
            totalWorkJoules: null,
            calories: null,
            trimp: null,
            powerLoad: null,
            heartRateLoad: null,
            paceLoad: null,
            strainScore: null,
          },
          details: {
            intervals: [],
            intervalGroups: [],
            streams: [],
            intervalSummary: [],
            skylineChart: [],
            powerZoneTimes: [],
            heartRateZoneTimes: [],
            paceZoneTimes: [],
            gapZoneTimes: [],
          },
        },
      ],
    };

    const onSelectWorkout = vi.fn();
    const { container } = render(<CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} />);

    const dayButtons = Array.from(container.querySelectorAll('.calendar-grid button')) as HTMLButtonElement[];
    const morningSpinButton = dayButtons.find((button) => button.textContent?.includes('Morning spin'));
    expect(morningSpinButton).toBeDefined();

    await userEvent.click(morningSpinButton as HTMLButtonElement);

    expect(onSelectWorkout).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      event: week.days[0].events[0],
      activity: week.days[0].activities[1],
    });
  });

  it('falls back to the visible activity when a completed event match is missing', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [{
        id: 15,
        startDateLocal: '2026-03-23',
        name: 'Completed workout',
        category: 'WORKOUT',
        description: null,
        indoor: false,
        color: null,
        eventDefinition: {
          rawWorkoutDoc: '- 20min 95%',
          intervals: [],
          segments: [],
          summary: {
            totalSegments: 0,
            totalDurationSeconds: 0,
            estimatedNormalizedPowerWatts: null,
            estimatedAveragePowerWatts: null,
            estimatedIntensityFactor: null,
            estimatedTrainingStressScore: null,
          },
        },
        actualWorkout: {
          activityId: 'a-missing',
          activityName: 'Matched ride',
          startDateLocal: '2026-03-23T08:30:00',
          powerValues: [],
          cadenceValues: [],
          heartRateValues: [],
          speedValues: [],
          averagePowerWatts: null,
          normalizedPowerWatts: null,
          trainingStressScore: null,
          intensityFactor: null,
          complianceScore: 0.8,
          matchedIntervals: [],
        },
      }],
      activities: [{
        id: 'a-other',
        startDateLocal: '2026-03-23T07:00:00',
        startDate: '2026-03-23T06:00:00Z',
        name: 'Morning spin',
        description: null,
        activityType: 'Ride',
        source: null,
        externalId: null,
        deviceName: null,
        distanceMeters: null,
        movingTimeSeconds: null,
        elapsedTimeSeconds: null,
        totalElevationGainMeters: null,
        averageSpeedMps: null,
        averageHeartRateBpm: null,
        averageCadenceRpm: null,
        trainer: false,
        commute: false,
        race: false,
        hasHeartRate: false,
        streamTypes: [],
        tags: [],
        metrics: {
          trainingStressScore: null,
          normalizedPowerWatts: null,
          intensityFactor: null,
          efficiencyFactor: null,
          variabilityIndex: null,
          averagePowerWatts: null,
          ftpWatts: null,
          totalWorkJoules: null,
          calories: null,
          trimp: null,
          powerLoad: null,
          heartRateLoad: null,
          paceLoad: null,
          strainScore: null,
        },
        details: {
          intervals: [],
          intervalGroups: [],
          streams: [],
          intervalSummary: [],
          skylineChart: [],
          powerZoneTimes: [],
          heartRateZoneTimes: [],
          paceZoneTimes: [],
          gapZoneTimes: [],
        },
      }],
    };

    const onSelectWorkout = vi.fn();
    const { container } = render(<CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} />);

    const morningSpinButton = Array.from(container.querySelectorAll('.calendar-grid button'))
      .find((button) => button.textContent?.includes('Morning spin')) as HTMLButtonElement | undefined;
    expect(morningSpinButton).toBeDefined();

    await userEvent.click(morningSpinButton as HTMLButtonElement);

    expect(onSelectWorkout).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      event: null,
      activity: week.days[0].activities[0],
    });
  });

  it('does not render training days as buttons when no selection handler is provided', () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [{
        id: 14,
        startDateLocal: '2026-03-23',
        name: 'Coach Build',
        category: 'WORKOUT',
        description: null,
        indoor: false,
        color: null,
        eventDefinition: {
          ...emptyEventDefinition(),
          summary: {
            totalSegments: 1,
            totalDurationSeconds: 2700,
            estimatedNormalizedPowerWatts: null,
            estimatedAveragePowerWatts: null,
            estimatedIntensityFactor: 0.8,
            estimatedTrainingStressScore: 42,
          },
        },
        actualWorkout: null,
      }],
      activities: [],
    };

    const { container } = render(<CalendarWeekSection week={week} />);

    expect(container.querySelector('.calendar-grid button')).toBeNull();
    expect(within(container).getByText('Coach Build')).toBeInTheDocument();
  });
});
