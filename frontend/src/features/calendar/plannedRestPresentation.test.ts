import { describe, expect, it } from 'vitest';

import { formatPlannedRestLabelSubtitle } from './plannedRestPresentation';
import type { CalendarPlannedRestDayLabel } from './types';

function makePlannedRestLabel(
  overrides: Partial<CalendarPlannedRestDayLabel['payload']> = {},
): CalendarPlannedRestDayLabel {
  return {
    kind: 'planned_rest_day',
    title: 'Planned rest',
    subtitle: null,
    payload: {
      plannedRestDayId: 'prd-1',
      startDate: '2026-07-01',
      endDate: '2026-07-07',
      title: null,
      note: null,
      ...overrides,
    },
  };
}

describe('formatPlannedRestLabelSubtitle', () => {
  it('formats each planned rest block from its own payload range', () => {
    const julyBlock = makePlannedRestLabel();
    const augustBlock = makePlannedRestLabel({
      plannedRestDayId: 'prd-2',
      startDate: '2026-08-01',
      endDate: '2026-08-08',
    });

    expect(formatPlannedRestLabelSubtitle(julyBlock, 'en')).toContain('Jul');
    expect(formatPlannedRestLabelSubtitle(julyBlock, 'en')).not.toContain('Aug');
    expect(formatPlannedRestLabelSubtitle(augustBlock, 'en')).toContain('Aug');
    expect(formatPlannedRestLabelSubtitle(augustBlock, 'en')).not.toContain('Jul');
  });
});
