import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import '../../../i18n';
import { CALENDAR_WEEK_ROW_HEIGHT } from '../constants';
import { CalendarLoadingRow } from './CalendarLoadingRow';

afterEach(() => {
  cleanup();
});

describe('CalendarLoadingRow', () => {
  it('renders translated fetching label for active loads', () => {
    render(<CalendarLoadingRow />);

    expect(screen.getByText(/fetching data/i)).toBeInTheDocument();
  });

  it('keeps idle placeholders visually quiet', () => {
    render(<CalendarLoadingRow status="idle" />);

    expect(screen.queryByText(/fetching data/i)).not.toBeInTheDocument();
  });

  it('derives its fixed height from the shared calendar row height', () => {
    const { container } = render(<CalendarLoadingRow />);

    expect(container.firstChild).toHaveStyle({ height: `${CALENDAR_WEEK_ROW_HEIGHT}px` });
  });

  it('renders an empty week shell instead of a message-heavy loading panel', () => {
    render(<CalendarLoadingRow />);

    expect(screen.getAllByText(/fetching data/i)).toHaveLength(1);
    expect(screen.queryByText(/upcoming training data/i)).not.toBeInTheDocument();
  });
});
