import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import '../../../i18n';
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
});
