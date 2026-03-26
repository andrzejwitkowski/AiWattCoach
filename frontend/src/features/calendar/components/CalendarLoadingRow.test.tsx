import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import '../../../i18n';
import { CALENDAR_WEEK_ROW_HEIGHT } from '../constants';
import { CalendarLoadingRow } from './CalendarLoadingRow';

afterEach(() => {
  cleanup();
});

describe('CalendarLoadingRow', () => {
  it('renders translated loading message', () => {
    render(<CalendarLoadingRow />);

    expect(screen.getByText(/loading week/i)).toBeInTheDocument();
  });

  it('does not expose each loading row as a live region', () => {
    render(<CalendarLoadingRow />);

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('derives its min height from the shared calendar row height', () => {
    const { container } = render(<CalendarLoadingRow />);

    expect(container.firstChild).toHaveStyle({ minHeight: `${CALENDAR_WEEK_ROW_HEIGHT}px` });
  });

  it('renders an incoming-week shell instead of a bare spinner box', () => {
    render(<CalendarLoadingRow />);

    expect(screen.getAllByText(/loading week/i)).toHaveLength(1);
    expect(screen.getByText(/upcoming training data/i)).toBeInTheDocument();
  });
});
