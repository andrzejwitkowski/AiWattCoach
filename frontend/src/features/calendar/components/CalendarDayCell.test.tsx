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
});
