import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import '../../../i18n';
import { CALENDAR_WEEK_ROW_HEIGHT } from '../constants';
import { CalendarLoadingRow } from './CalendarLoadingRow';

describe('CalendarLoadingRow', () => {
  it('renders translated loading message', () => {
    render(<CalendarLoadingRow />);

    expect(screen.getByText(/retrieving events/i)).toBeInTheDocument();
  });

  it('does not expose each loading row as a live region', () => {
    render(<CalendarLoadingRow />);

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('derives its min height from the shared calendar row height', () => {
    const { container } = render(<CalendarLoadingRow />);

    expect(container.firstChild).toHaveStyle({ minHeight: `${CALENDAR_WEEK_ROW_HEIGHT}px` });
  });
});
