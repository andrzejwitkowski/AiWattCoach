import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { ParsedStableContext } from '../utils/parseStableContext';
import { StableContextBody } from './StableContextBody';

const parsed: ParsedStableContext = {
  workoutId: null,
  rpe: null,
  workoutDate: null,
  workoutContext: null,
  workoutRecap: null,
  athleteSummary: null,
  mesoWindowStart: null,
  mesoWindowEnd: null,
  mesoRoadmapGuidance: 'Predicted roadmap guidance.',
  mesoRoadmap: {
    windowStart: '2026-06-09',
    windowEnd: '2026-06-10',
    days: [{ date: '2026-06-09', restDay: true, restDayReason: 'Recovery' }],
  },
  savedAtEpochSeconds: null,
  calendarConversation: null,
  packed: null,
};

describe('StableContextBody', () => {
  it('shows meso roadmap raw json toggle', () => {
    render(<StableContextBody parsed={parsed} packedLabel="Training Context" />);

    fireEvent.click(screen.getByText(/meso cycle roadmap \(predicted\)/i));
    fireEvent.click(screen.getByRole('button', { name: /show raw json/i }));

    expect(screen.getByText(/"windowStart": "2026-06-09"/)).toBeInTheDocument();
    expect(screen.getByText(/"restDayReason": "Recovery"/)).toBeInTheDocument();
  });
});
