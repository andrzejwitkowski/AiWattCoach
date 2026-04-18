import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { createFetchMock, useFetchMock } from '../features/intervals/api/testHelpers';
import { AppHomePage } from './AppHomePage';

function buildResponse(range: '90d' | 'season' | 'all-time') {
  const summary = range === 'season'
    ? {
        currentCtl: 41.2,
        currentAtl: 46.1,
        currentTsb: 4.9,
        ftpWatts: 340,
        averageIf28d: 76.1,
        averageEf28d: 1.91,
        loadDeltaCtl14d: 4.3,
        tsbZone: 'freshness_peak' as const,
      }
    : {
        currentCtl: 29.9,
        currentAtl: 53.4,
        currentTsb: -23.5,
        ftpWatts: 340,
        averageIf28d: 72.95,
        averageEf28d: null,
        loadDeltaCtl14d: 8.4,
        tsbZone: 'optimal_training' as const,
      };

  return new Response(
    JSON.stringify({
      range,
      windowStart: range === 'all-time' ? '2025-12-03' : range === 'season' ? '2026-01-01' : '2026-01-19',
      windowEnd: '2026-04-18',
      hasTrainingLoad: true,
      summary,
      points: [
        {
          date: '2026-04-01',
          dailyTss: 44,
          currentCtl: range === 'season' ? 34.8 : 20.0,
          currentAtl: range === 'season' ? 40.2 : 30.0,
          currentTsb: range === 'season' ? 1.8 : -10.0,
        },
        {
          date: '2026-04-18',
          dailyTss: 97,
          currentCtl: summary.currentCtl,
          currentAtl: summary.currentAtl,
          currentTsb: summary.currentTsb,
        },
      ],
    }),
    { status: 200, headers: { 'content-type': 'application/json' } },
  );
}

describe('AppHomePage', () => {
  it('loads dashboard and switches ranges', async () => {
    const fetchMock = useFetchMock(
      createFetchMock()
        .mockResolvedValueOnce(buildResponse('90d'))
        .mockResolvedValueOnce(buildResponse('season')),
    );

    render(<AppHomePage apiBaseUrl="" />);

    expect(screen.getByText(/loading dashboard/i)).toBeInTheDocument();
    await screen.findByRole('heading', { name: /training stress and recovery/i });
    expect(screen.getByText('29.9')).toBeInTheDocument();
    expect(screen.getByText(/understanding form \(tsb\)/i)).toBeInTheDocument();
    expect(screen.getByText(/coach insight/i)).toBeInTheDocument();
    expect(screen.getAllByText(/latest snapshot/i).length).toBeGreaterThan(0);

    await userEvent.click(screen.getByRole('radio', { name: /season/i }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenLastCalledWith(
        '/api/dashboard/training-load?range=season',
        expect.any(Object),
      );
    });

    expect(await screen.findByText('41.2')).toBeInTheDocument();
    expect(screen.getByText(/trending towards peak/i)).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /season/i })).toBeChecked();
  });

  it('supports keyboard range changes', async () => {
    useFetchMock(
      createFetchMock()
        .mockResolvedValueOnce(buildResponse('90d'))
        .mockResolvedValueOnce(buildResponse('season')),
    );

    const user = userEvent.setup();

    const { container } = render(<AppHomePage apiBaseUrl="" />);

    await within(container).findByRole('heading', { name: /training stress and recovery/i });

    const ninetyDays = screen.getByRole('radio', { name: /90 days/i });
    ninetyDays.focus();
    await user.keyboard('{ArrowRight}');

    expect(await screen.findByText('41.2')).toBeInTheDocument();
    expect(screen.getByText(/trending towards peak/i)).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /season/i })).toBeChecked();
  });

  it('shows snapshot values on chart hover instead of keeping the tsb tooltip always visible', async () => {
    const user = userEvent.setup();

    useFetchMock(
      createFetchMock().mockResolvedValueOnce(buildResponse('90d')),
    );

    const { container } = render(<AppHomePage apiBaseUrl="" />);

    const tsbChart = await within(container).findByLabelText(/form chart with freshness, optimal training, and high risk zones/i);
    const tsbSection = tsbChart.closest('section');

    expect(tsbSection).not.toBeNull();

    const tsbQueries = within(tsbSection!);

    expect(tsbQueries.queryByText(/^latest snapshot$/i)).not.toBeInTheDocument();
    expect(tsbQueries.getAllByText('-23.5')).toHaveLength(1);

    await user.hover(tsbChart);

    expect(await tsbQueries.findByText(/^latest snapshot$/i)).toBeInTheDocument();
    expect(tsbQueries.getAllByText('-23.5')).toHaveLength(2);
  });

  it('renders empty state when report has no snapshots', async () => {
    useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            range: 'all-time',
            windowStart: '2026-04-18',
            windowEnd: '2026-04-18',
            hasTrainingLoad: false,
            summary: {
              currentCtl: null,
              currentAtl: null,
              currentTsb: null,
              ftpWatts: null,
              averageIf28d: null,
              averageEf28d: null,
              loadDeltaCtl14d: null,
              tsbZone: 'optimal_training',
            },
            points: [],
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    render(<AppHomePage apiBaseUrl="" />);

    expect(await screen.findByText(/training load will appear here/i)).toBeInTheDocument();
  });
});
