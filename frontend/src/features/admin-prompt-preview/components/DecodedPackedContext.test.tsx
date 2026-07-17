import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { DecodedPackedContext } from './DecodedPackedContext';

describe('DecodedPackedContext', () => {
  it('renders meso roadmap days instead of collapsing them to a count', () => {
    render(
      <DecodedPackedContext
        label="Meso Cycle Roadmap"
        data={{
          windowStart: '2026-06-09',
          windowEnd: '2026-06-10',
          days: [
            { date: '2026-06-09', restDay: true, restDayReason: 'Recovery' },
            { date: '2026-06-10', restDay: false, name: 'Endurance Ride' },
          ],
        }}
      />,
    );

    expect(screen.getByText('Planned Days (2)')).toBeInTheDocument();
    expect(screen.getByText('Endurance Ride')).toBeInTheDocument();
    expect(screen.getByText('Recovery')).toBeInTheDocument();
    expect(screen.queryByText('[2 items]')).not.toBeInTheDocument();
  });

  it('renders planned rest day blocks instead of collapsing them to a count', () => {
    render(
      <DecodedPackedContext
        label="Stable Context"
        data={{
          v: 1,
          prd: [
            {
              id: 'prd-1',
              sd: '2026-07-01',
              ed: '2026-07-07',
              n: 'Summer break',
              nt: 'No structured training',
            },
            {
              id: 'prd-2',
              sd: '2026-08-01',
              ed: '2026-08-01',
              n: 'Travel day',
            },
          ],
        }}
      />,
    );

    expect(screen.getByText('Planned Rest Days (2)')).toBeInTheDocument();
    expect(screen.getByText('Summer break')).toBeInTheDocument();
    expect(screen.getByText('2026-07-01 – 2026-07-07 (7 days)')).toBeInTheDocument();
    expect(screen.getByText('No structured training')).toBeInTheDocument();
    expect(screen.getByText('Travel day')).toBeInTheDocument();
    expect(screen.getByText('2026-08-01')).toBeInTheDocument();
    expect(screen.queryByText('Other Fields')).not.toBeInTheDocument();
    expect(screen.queryByText('[2 items]')).not.toBeInTheDocument();
  });

  it('renders historical workouts with ps and cs segments', () => {
    render(
      <DecodedPackedContext
        label="Stable Context"
        data={{
          h: {
            ac: 1,
            w: [
              {
                d: '2026-03-20',
                id: 'ride-1',
                n: 'Sweet Spot',
                ps: [[220, 220, 180], [270, 270, 120]],
                cs: [[84, 84, 300]],
              },
            ],
          },
        }}
      />,
    );

    fireEvent.click(screen.getByText(/historical workouts \(1\)/i));
    expect(screen.getByText('Sweet Spot')).toBeInTheDocument();
    expect(screen.getByText('220 W · 3m')).toBeInTheDocument();
    expect(screen.getByText('270 W · 2m')).toBeInTheDocument();
    expect(screen.getByText('84 RPM · 5m')).toBeInTheDocument();
    expect(screen.queryByText('[1 items]')).not.toBeInTheDocument();
  });

  it('renders recent days from header-mapped workout tables instead of labeling them rest', () => {
    render(
      <DecodedPackedContext
        label="Volatile Training Context"
        data={{
          rd: [
            {
              d: '2026-07-15',
              fr: false,
              w: {
                h: ['id', 'sd', 'n', 'tss'],
                r: [['ride-1', '2026-07-15T10:00:00', 'Endurance', 72]],
              },
            },
          ],
        }}
      />,
    );

    expect(screen.getByText('1 workout')).toBeInTheDocument();
    fireEvent.click(screen.getByText('2026-07-15'));
    expect(screen.getByText('Endurance')).toBeInTheDocument();
    expect(screen.getByText('72 TSS')).toBeInTheDocument();
    expect(screen.queryByText('Rest', { selector: 'summary span' })).not.toBeInTheDocument();
  });

  it('renders race strategy table instead of collapsing rs to other fields', () => {
    const { container } = render(
      <DecodedPackedContext
        label="Volatile Training Context"
        data={{
          rs: {
            h: ['d', 'pri', 'disc', 'n', 'days_out'],
            r: [['2026-07-20', 'A', 'road', 'Szosomania', 4]],
          },
        }}
      />,
    );

    expect(screen.getByText('Race Strategy (1)')).toBeInTheDocument();
    expect(screen.getByText('Szosomania')).toBeInTheDocument();
    expect(screen.getByText('road')).toBeInTheDocument();
    expect(screen.getByText('4')).toBeInTheDocument();
    expect(container.textContent).not.toContain('Other Fields');
  });

  it('renders upcoming days from header-mapped planned tables', () => {
    render(
      <DecodedPackedContext
        label="Volatile Training Context"
        data={{
          ud: [
            {
              d: '2026-07-18',
              fr: false,
              pw: {
                h: ['id', 'n', 'tss'],
                r: [[101, 'Easy spin', 40]],
              },
            },
          ],
        }}
      />,
    );

    expect(screen.getByText('Upcoming Days')).toBeInTheDocument();
    expect(screen.getByText('1 planned')).toBeInTheDocument();
    fireEvent.click(screen.getByText('2026-07-18'));
    expect(screen.getByText('Easy spin')).toBeInTheDocument();
  });

  it('renders aligned intervals map instead of collapsing sa to other fields', () => {
    const { container } = render(
      <DecodedPackedContext
        label="Volatile Training Context"
        data={{
          sa: {
            'ride-1': [
              {
                interval_index: 0,
                planned_step: {
                  step_type: 'work',
                  target_power_min: 200,
                  target_power_max: 220,
                  planned_duration_seconds: 600,
                },
                actual_duration_seconds: 580,
                normalized_power: 215,
              },
            ],
          },
        }}
      />,
    );

    expect(screen.getByText('Aligned Intervals (1 workout)')).toBeInTheDocument();
    expect(screen.getByText('ride-1')).toBeInTheDocument();
    expect(screen.getByText('1 interval')).toBeInTheDocument();
    expect(container.textContent).not.toContain('Other Fields');
  });
});
