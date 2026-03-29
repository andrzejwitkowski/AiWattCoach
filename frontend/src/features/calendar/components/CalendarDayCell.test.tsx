import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import '../../../i18n';
import type { CalendarDay } from '../types';
import { CalendarDayCell } from './CalendarDayCell';

function emptyWorkoutSummary() {
  return {
    totalSegments: 0,
    totalDurationSeconds: 0,
    estimatedNormalizedPowerWatts: null,
    estimatedAveragePowerWatts: null,
    estimatedIntensityFactor: null,
    estimatedTrainingStressScore: null,
  };
}

function emptyEventDefinition() {
  return {
    rawWorkoutDoc: null,
    intervals: [],
    segments: [],
    summary: emptyWorkoutSummary(),
  };
}

function intervalDefinition(definition: string) {
  return {
    definition,
    repeatCount: 1,
    durationSeconds: null,
    targetPercentFtp: null,
    zoneId: null,
  };
}

describe('CalendarDayCell', () => {
  it('renders a rest day state when no data is present', () => {
    const day: CalendarDay = {
      date: new Date(2026, 2, 23),
      dateKey: '2026-03-23',
      events: [],
      activities: [],
    };

    render(<CalendarDayCell day={day} isToday={false} />);

    expect(screen.getByText(/rest day/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /rest day/i })).not.toBeInTheDocument();
  });

  it('renders workout content when activity data exists', () => {
    const day: CalendarDay = {
      date: new Date(2026, 2, 24),
      dateKey: '2026-03-24',
      events: [],
      activities: [
        {
          id: 'a1',
          startDateLocal: '2026-03-24T08:00:00',
          startDate: '2026-03-24T07:00:00Z',
          name: 'Morning Ride',
          description: null,
          activityType: 'Ride',
          source: null,
          externalId: null,
          deviceName: null,
          distanceMeters: 40000,
          movingTimeSeconds: 5400,
          elapsedTimeSeconds: 5600,
          totalElevationGainMeters: null,
          averageSpeedMps: null,
          averageHeartRateBpm: null,
          averageCadenceRpm: null,
          trainer: false,
          commute: false,
          race: false,
          hasHeartRate: true,
          streamTypes: [],
          tags: [],
          metrics: {
            trainingStressScore: 88,
            normalizedPowerWatts: null,
            intensityFactor: null,
            efficiencyFactor: null,
            variabilityIndex: null,
            averagePowerWatts: null,
            ftpWatts: null,
            totalWorkJoules: null,
            calories: 950,
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

    render(<CalendarDayCell day={day} isToday={true} />);

    expect(screen.getByText('Morning Ride')).toBeInTheDocument();
    expect(screen.getByText(/90 min/i)).toBeInTheDocument();
    expect(screen.getByText(/88 TSS/i)).toBeInTheDocument();
  });

  it('shows a more-items indicator when multiple items exist on one day', () => {
    const day: CalendarDay = {
      date: new Date(2026, 2, 24),
      dateKey: '2026-03-24',
      events: [
        {
          id: 2,
          startDateLocal: '2026-03-24',
          name: 'Planned ride',
          category: 'WORKOUT',
          description: null,
          indoor: false,
          color: null,
          eventDefinition: emptyEventDefinition(),
          actualWorkout: null,
        },
      ],
      activities: [
        {
          id: 'a1',
          startDateLocal: '2026-03-24T08:00:00',
          startDate: '2026-03-24T07:00:00Z',
          name: 'Morning Ride',
          description: null,
          activityType: 'Ride',
          source: null,
          externalId: null,
          deviceName: null,
          distanceMeters: 40000,
          movingTimeSeconds: 5400,
          elapsedTimeSeconds: 5600,
          totalElevationGainMeters: null,
          averageSpeedMps: null,
          averageHeartRateBpm: null,
          averageCadenceRpm: null,
          trainer: false,
          commute: false,
          race: false,
          hasHeartRate: true,
          streamTypes: [],
          tags: [],
          metrics: {
            trainingStressScore: 88,
            normalizedPowerWatts: null,
            intensityFactor: null,
            efficiencyFactor: null,
            variabilityIndex: null,
            averagePowerWatts: null,
            ftpWatts: null,
            totalWorkJoules: null,
            calories: 950,
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

    render(<CalendarDayCell day={day} isToday={false} />);

    expect(screen.getByText('+1 more item')).toBeInTheDocument();
  });

  it('does not label unnamed training activity as a rest day', () => {
    const day: CalendarDay = {
      date: new Date(2026, 2, 25),
      dateKey: '2026-03-25',
      events: [],
      activities: [
        {
          id: 'a2',
          startDateLocal: '2026-03-25T08:00:00',
          startDate: '2026-03-25T07:00:00Z',
          name: null,
          description: null,
          activityType: 'Ride',
          source: null,
          externalId: null,
          deviceName: null,
          distanceMeters: 32000,
          movingTimeSeconds: 3600,
          elapsedTimeSeconds: 3700,
          totalElevationGainMeters: null,
          averageSpeedMps: null,
          averageHeartRateBpm: null,
          averageCadenceRpm: null,
          trainer: false,
          commute: false,
          race: false,
          hasHeartRate: true,
          streamTypes: [],
          tags: [],
          metrics: {
            trainingStressScore: 55,
            normalizedPowerWatts: null,
            intensityFactor: null,
            efficiencyFactor: null,
            variabilityIndex: null,
            averagePowerWatts: null,
            ftpWatts: null,
            totalWorkJoules: null,
            calories: 700,
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

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);

    expect(screen.getByText('Ride')).toBeInTheDocument();
    expect(container).not.toHaveTextContent(/^Rest Day$/i);
  });

  it('keeps the mini chart aligned with the displayed primary activity', () => {
    const day: CalendarDay = {
      date: new Date(2026, 2, 26),
      dateKey: '2026-03-26',
      events: [
        {
          id: 3,
          startDateLocal: '2026-03-26',
          name: 'Planned intervals',
          category: 'WORKOUT',
          description: null,
          indoor: false,
          color: null,
          eventDefinition: {
            rawWorkoutDoc: null,
            intervals: [
              intervalDefinition('10m'),
              intervalDefinition('8m'),
              intervalDefinition('6m'),
              intervalDefinition('4m'),
            ],
            segments: [],
            summary: emptyWorkoutSummary(),
          },
          actualWorkout: null,
        },
      ],
      activities: [
        {
          id: 'a3',
          startDateLocal: '2026-03-26T08:00:00',
          startDate: '2026-03-26T07:00:00Z',
          name: 'Morning Ride',
          description: null,
          activityType: 'Ride',
          source: null,
          externalId: null,
          deviceName: null,
          distanceMeters: 40000,
          movingTimeSeconds: 5400,
          elapsedTimeSeconds: 5600,
          totalElevationGainMeters: null,
          averageSpeedMps: null,
          averageHeartRateBpm: null,
          averageCadenceRpm: null,
          trainer: false,
          commute: false,
          race: false,
          hasHeartRate: true,
          streamTypes: [],
          tags: [],
          metrics: {
            trainingStressScore: 88,
            normalizedPowerWatts: null,
            intensityFactor: null,
            efficiencyFactor: null,
            variabilityIndex: null,
            averagePowerWatts: null,
            ftpWatts: null,
            totalWorkJoules: null,
            calories: 950,
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

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);

    expect(container).toHaveTextContent('Morning Ride');
    expect(container.querySelectorAll('div[style*="height"]').length).toBe(4);
  });

  it('uses the primary activity type as subtitle fallback when activity metrics are missing', () => {
    const day: CalendarDay = {
      date: new Date(2026, 2, 27),
      dateKey: '2026-03-27',
      events: [
        {
          id: 4,
          startDateLocal: '2026-03-27',
          name: 'Planned swim set',
          category: 'SWIM',
          description: null,
          indoor: false,
          color: null,
          eventDefinition: emptyEventDefinition(),
          actualWorkout: null,
        },
      ],
      activities: [
        {
          id: 'a4',
          startDateLocal: '2026-03-27T08:00:00',
          startDate: '2026-03-27T07:00:00Z',
          name: 'Evening Ride',
          description: null,
          activityType: 'Ride',
          source: null,
          externalId: null,
          deviceName: null,
          distanceMeters: 0,
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

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);

    expect(container).toHaveTextContent('Evening Ride');
    expect(container).toHaveTextContent('Ride');
    expect(container).not.toHaveTextContent('Swim');
  });

  it('renders planned mini-chart bars with their workout zone colors', () => {
    const day: CalendarDay = {
      date: new Date(2026, 2, 28),
      dateKey: '2026-03-28',
      activities: [],
      events: [
        {
          id: 5,
          startDateLocal: '2026-03-28',
          name: 'Threshold set',
          category: 'WORKOUT',
          description: null,
          indoor: true,
          color: null,
          eventDefinition: {
            rawWorkoutDoc: '10m Z2\n5m Z4\n2m Z6',
            intervals: [
              { definition: '10m Z2', repeatCount: 1, durationSeconds: 600, targetPercentFtp: 70, zoneId: 2 },
              { definition: '5m Z4', repeatCount: 1, durationSeconds: 300, targetPercentFtp: 100, zoneId: 4 },
              { definition: '2m Z6', repeatCount: 1, durationSeconds: 120, targetPercentFtp: 130, zoneId: 6 },
            ],
            segments: [
              { order: 1, label: 'Warmup', durationSeconds: 600, startOffsetSeconds: 0, endOffsetSeconds: 600, targetPercentFtp: 70, zoneId: 2 },
              { order: 2, label: 'Threshold', durationSeconds: 300, startOffsetSeconds: 600, endOffsetSeconds: 900, targetPercentFtp: 100, zoneId: 4 },
              { order: 3, label: 'VO2', durationSeconds: 120, startOffsetSeconds: 900, endOffsetSeconds: 1020, targetPercentFtp: 130, zoneId: 6 },
            ],
            summary: {
              totalSegments: 3,
              totalDurationSeconds: 1020,
              estimatedNormalizedPowerWatts: null,
              estimatedAveragePowerWatts: null,
              estimatedIntensityFactor: 0.88,
              estimatedTrainingStressScore: 75,
            },
          },
          actualWorkout: null,
        },
      ],
    };

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);

    const chartBars = Array.from(container.querySelectorAll('div[style*="height"]'));
    const backgroundColors = chartBars.map((bar) => (bar as HTMLDivElement).style.backgroundColor).filter(Boolean);

    expect(chartBars.length).toBe(3);
    expect(new Set(backgroundColors).size).toBeGreaterThan(1);
    expect(backgroundColors).toContain('rgb(0, 227, 253)');
    expect(backgroundColors).toContain('rgb(210, 255, 154)');
    expect(backgroundColors).toContain('rgb(255, 115, 81)');
  });
});
