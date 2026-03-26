import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import '../../../i18n';
import { CalendarPerformanceCards } from './CalendarPerformanceCards';

describe('CalendarPerformanceCards', () => {
  it('does not present fabricated performance metrics as real values', () => {
    render(<CalendarPerformanceCards />);

    expect(screen.queryByText('94')).not.toBeInTheDocument();
    expect(screen.queryByText('112')).not.toBeInTheDocument();
    expect(screen.queryByText('-18')).not.toBeInTheDocument();
    expect(screen.getAllByText(/coming soon/i).length).toBeGreaterThanOrEqual(4);
  });
});
