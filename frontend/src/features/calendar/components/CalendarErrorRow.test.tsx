import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import '../../../i18n';
import { CALENDAR_WEEK_ROW_HEIGHT } from '../constants';
import { CalendarErrorRow } from './CalendarErrorRow';

describe('CalendarErrorRow', () => {
  it('renders the translated error copy', () => {
    render(<CalendarErrorRow />);

    expect(screen.getByText(/week data unavailable/i)).toBeInTheDocument();
  });

  it('derives its min height from the shared calendar row height', () => {
    const { container } = render(<CalendarErrorRow />);

    expect(container.firstChild).toHaveStyle({ minHeight: `${CALENDAR_WEEK_ROW_HEIGHT}px` });
  });
});
