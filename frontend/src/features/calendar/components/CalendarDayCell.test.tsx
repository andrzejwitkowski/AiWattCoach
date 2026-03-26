import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import '../../../i18n';
import type { CalendarDay } from '../types';
import { CalendarDayCell } from './CalendarDayCell';

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
          eventDefinition: {
            rawWorkoutDoc: null,
            intervals: [],
          },
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
              { definition: '10m' },
              { definition: '8m' },
              { definition: '6m' },
              { definition: '4m' },
            ],
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
    expect(container.querySelectorAll('div[style*="height"]').length).toBe(3);
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
          eventDefinition: {
            rawWorkoutDoc: null,
            intervals: [],
          },
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
});
