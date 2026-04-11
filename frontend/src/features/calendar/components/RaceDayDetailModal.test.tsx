import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { RaceDayDetailModal } from './RaceDayDetailModal';

describe('RaceDayDetailModal', () => {
  it('uses a race-specific close label and closes on Escape', () => {
    const onClose = vi.fn();

    render(
      <RaceDayDetailModal
        selection={{
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
        }}
        onClose={onClose}
      />,
    );

    expect(screen.getByRole('button', { name: /close race details/i })).toBeInTheDocument();

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
