import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { DayItemsModal } from './DayItemsModal';

describe('DayItemsModal', () => {
  it('closes on Escape and traps focus within the dialog', async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();

    render(
      <>
        <button type="button">Background action</button>
        <DayItemsModal
          selection={{
            dateKey: '2026-03-23',
            items: [
              {
                kind: 'planned',
                id: 'planned:1',
                title: 'Opener',
                subtitle: '20 min',
                dateKey: '2026-03-23',
                priorityRank: 1,
                tss: 16,
                event: {
                  id: 1,
                  calendarEntryId: 'intervals:1',
                  startDateLocal: '2026-03-23',
                  name: 'Opener',
                  category: 'WORKOUT',
                  description: null,
                  indoor: false,
                  color: null,
                  eventDefinition: {
                    rawWorkoutDoc: null,
                    intervals: [],
                    segments: [],
                    summary: {
                      totalSegments: 0,
                      totalDurationSeconds: 1200,
                      estimatedNormalizedPowerWatts: null,
                      estimatedAveragePowerWatts: null,
                      estimatedIntensityFactor: null,
                      estimatedTrainingStressScore: 16,
                    },
                  },
                  actualWorkout: null,
                  plannedSource: 'intervals',
                  syncStatus: null,
                  linkedIntervalsEventId: null,
                  projectedWorkout: null,
                },
              },
            ],
          }}
          onClose={onClose}
          onSelectItem={vi.fn()}
        />
      </>,
    );

    const closeButton = screen.getByRole('button', { name: /close day items/i });
    expect(closeButton).toHaveFocus();

    await user.tab();
    expect(screen.getByRole('button', { name: /planned workout opener 20 min/i })).toHaveFocus();

    await user.tab();
    expect(closeButton).toHaveFocus();

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
