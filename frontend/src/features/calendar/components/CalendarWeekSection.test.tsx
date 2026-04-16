import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import type { CalendarWeek } from '../types';
import { CalendarWeekSection } from './CalendarWeekSection';

vi.mock('react-i18next', async () => {
  const actual = await vi.importActual<typeof import('react-i18next')>('react-i18next');

  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) => {
        switch (key) {
          case 'calendar.fetchingData':
            return 'Fetching data';
          case 'calendar.plannedWorkout':
            return 'Planned Workout';
          case 'calendar.workout':
            return 'Workout';
          default:
            return key;
        }
      },
      i18n: {
        resolvedLanguage: 'en',
        language: 'en',
      },
    }),
  };
});

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
    const onSelectDayItems = vi.fn();
    const { container } = render(
      <CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} onSelectDayItems={onSelectDayItems} />,
    );

    const dayButton = container.querySelector('.calendar-grid button') as HTMLButtonElement | null;
    expect(dayButton).toBeTruthy();
    await userEvent.click(dayButton as HTMLButtonElement);

    expect(onSelectWorkout).not.toHaveBeenCalled();
    expect(onSelectDayItems).toHaveBeenCalledTimes(1);
    expect(onSelectDayItems).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      items: expect.arrayContaining([
        expect.objectContaining({ kind: 'planned', title: 'Planned workout' }),
        expect.objectContaining({ kind: 'completed', title: 'Unrelated ride' }),
      ]),
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

  it('does not open workout details for a single generic non-workout event', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [{
        id: 77,
        startDateLocal: '2026-03-23',
        name: 'Travel note',
        category: 'NOTE',
        description: 'Bring spare wheels',
        indoor: false,
        color: null,
        eventDefinition: emptyEventDefinition(),
        actualWorkout: null,
        plannedSource: 'intervals',
        syncStatus: null,
        linkedIntervalsEventId: null,
        projectedWorkout: null,
      }],
      activities: [],
    };

    const onSelectWorkout = vi.fn();
    const onSelectDayItems = vi.fn();
    render(<CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} onSelectDayItems={onSelectDayItems} />);

    expect(screen.queryByRole('button', { name: /travel note/i })).not.toBeInTheDocument();

    expect(onSelectWorkout).not.toHaveBeenCalled();
    expect(onSelectDayItems).not.toHaveBeenCalled();
  });

  it('does not open the day-items picker for multiple generic non-workout events', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [
        {
          id: 78,
          startDateLocal: '2026-03-23',
          name: 'Travel note',
          category: 'NOTE',
          description: 'Bring spare wheels',
          indoor: false,
          color: null,
          eventDefinition: emptyEventDefinition(),
          actualWorkout: null,
          plannedSource: 'intervals',
          syncStatus: null,
          linkedIntervalsEventId: null,
          projectedWorkout: null,
        },
        {
          id: 79,
          startDateLocal: '2026-03-23',
          name: 'Logistics',
          category: 'NOTE',
          description: 'Confirm hotel',
          indoor: false,
          color: null,
          eventDefinition: emptyEventDefinition(),
          actualWorkout: null,
          plannedSource: 'intervals',
          syncStatus: null,
          linkedIntervalsEventId: null,
          projectedWorkout: null,
        },
      ],
      activities: [],
    };

    const onSelectWorkout = vi.fn();
    const onSelectDayItems = vi.fn();
    render(<CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} onSelectDayItems={onSelectDayItems} />);

    expect(screen.queryByRole('button', { name: /travel note/i })).not.toBeInTheDocument();
    expect(onSelectWorkout).not.toHaveBeenCalled();
    expect(onSelectDayItems).not.toHaveBeenCalled();
  });

  it('routes multi-item days to the day-items picker instead of direct workout details', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [
        {
          id: 99,
          startDateLocal: '2026-03-23',
          name: 'Race day',
          category: 'RACE',
          description: null,
          indoor: false,
          color: null,
          eventDefinition: emptyEventDefinition(),
          actualWorkout: null,
          plannedSource: 'intervals',
          syncStatus: null,
          linkedIntervalsEventId: 99,
          projectedWorkout: null,
        },
        {
          id: 12,
          startDateLocal: '2026-03-23',
          name: 'Opener',
          category: 'WORKOUT',
          description: null,
          indoor: false,
          color: null,
          eventDefinition: {
            rawWorkoutDoc: '- 20min 70%',
            intervals: [],
            segments: [],
            summary: {
              totalSegments: 1,
              totalDurationSeconds: 1200,
              estimatedNormalizedPowerWatts: null,
              estimatedAveragePowerWatts: null,
              estimatedIntensityFactor: 0.7,
              estimatedTrainingStressScore: 16,
            },
          },
          actualWorkout: null,
          plannedSource: 'intervals',
          syncStatus: null,
          linkedIntervalsEventId: null,
          projectedWorkout: null,
        },
      ],
      labels: [
        {
          kind: 'race',
          title: 'Race day',
          subtitle: '52 km • Kat. B',
          payload: {
            raceId: 'race-1',
            date: '2026-03-23',
            name: 'Grojec',
            distanceMeters: 52000,
            discipline: 'road',
            priority: 'B',
            syncStatus: 'synced',
            linkedIntervalsEventId: 99,
          },
        },
      ],
      activities: [],
    };

    const onSelectWorkout = vi.fn();
    const onSelectDayItems = vi.fn();
    render(<CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} onSelectDayItems={onSelectDayItems} />);

    await userEvent.click(screen.getByRole('button', { name: /grojec/i }));

    expect(onSelectWorkout).not.toHaveBeenCalled();
    expect(onSelectDayItems).toHaveBeenCalledTimes(1);
    expect(onSelectDayItems.mock.calls[0]?.[0].items).toHaveLength(2);
  });

  it('falls back to direct workout selection on multi-item days when no picker handler is provided', async () => {
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
        eventDefinition: emptyEventDefinition(),
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
    const { container } = render(<CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} />);
    const dayButtons = Array.from(container.querySelectorAll('.calendar-grid button')) as HTMLButtonElement[];
    const unrelatedRideButton = dayButtons.find((button) => button.textContent?.includes('Unrelated ride'));

    expect(unrelatedRideButton).toBeDefined();
    await userEvent.click(unrelatedRideButton as HTMLButtonElement);

    expect(onSelectWorkout).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      event: null,
      activity: week.days[0].activities[0],
    });
  });

  it('selects the planned event and matched activity when a planned workout includes actual workout data', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [{
        id: 12,
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
    const { container } = render(<CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} />);

    const dayButton = container.querySelector('.calendar-grid button') as HTMLButtonElement | null;
    expect(dayButton).toBeTruthy();
    await userEvent.click(dayButton as HTMLButtonElement);

    expect(onSelectWorkout).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      event: week.days[0].events[0],
      activity: week.days[0].activities[0],
    });
  });

  it('selects the planned event with the matched activity even when it is not the first activity of the day', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [{
        id: 13,
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
    const onSelectDayItems = vi.fn();
    const { container } = render(
      <CalendarWeekSection week={week} onSelectWorkout={onSelectWorkout} onSelectDayItems={onSelectDayItems} />,
    );

    const dayButton = container.querySelector('.calendar-grid button') as HTMLButtonElement | null;
    expect(dayButton).toBeTruthy();

    await userEvent.click(dayButton as HTMLButtonElement);

    expect(onSelectWorkout).not.toHaveBeenCalled();
    expect(onSelectDayItems).toHaveBeenCalledTimes(1);
    expect(onSelectDayItems).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      items: expect.arrayContaining([
        expect.objectContaining({ kind: 'planned', title: 'Planned workout' }),
        expect.objectContaining({ kind: 'completed', title: 'Morning spin' }),
      ]),
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

    const dayButton = container.querySelector('.calendar-grid button') as HTMLButtonElement | null;
    expect(dayButton).toBeTruthy();

    await userEvent.click(dayButton as HTMLButtonElement);

    expect(onSelectWorkout).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      event: null,
      activity: week.days[0].activities[0],
    });
  });

  it('falls back to a later planned-vs-actual event even when a generic note appears first', async () => {
    const week = createWeek('loaded');
    week.days[0] = {
      ...week.days[0],
      events: [
        {
          id: 16,
          startDateLocal: '2026-03-23',
          name: 'Travel note',
          category: 'NOTE',
          description: 'Bring spare wheels',
          indoor: false,
          color: null,
          eventDefinition: emptyEventDefinition(),
          actualWorkout: null,
          plannedSource: 'intervals',
          syncStatus: null,
          linkedIntervalsEventId: null,
          projectedWorkout: null,
        },
        {
          id: 17,
          startDateLocal: '2026-03-23',
          name: 'Planned workout',
          category: 'WORKOUT',
          description: null,
          indoor: false,
          color: null,
          eventDefinition: emptyEventDefinition(),
          actualWorkout: {
            activityId: 'a-other',
            activityName: 'Morning spin',
            startDateLocal: '2026-03-23T07:00:00',
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
          plannedSource: 'intervals',
          syncStatus: null,
          linkedIntervalsEventId: null,
          projectedWorkout: null,
        },
      ],
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

    const dayButton = container.querySelector('.calendar-grid button') as HTMLButtonElement | null;
    expect(dayButton).toBeTruthy();

    await userEvent.click(dayButton as HTMLButtonElement);

    expect(onSelectWorkout).toHaveBeenCalledWith({
      dateKey: '2026-03-23',
      event: week.days[0].events[1],
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
