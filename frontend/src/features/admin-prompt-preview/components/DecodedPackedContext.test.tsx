import { render, screen } from '@testing-library/react';
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
});
