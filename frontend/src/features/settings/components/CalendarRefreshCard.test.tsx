import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { CalendarRefreshCard } from './CalendarRefreshCard';
import { refreshCalendarView } from '../../calendar/api/calendar';
import { invalidateCalendarCache } from '../../calendar/hooks/useCalendarData';
import { AuthenticationError } from '../../../lib/httpClient';

vi.mock('../../calendar/api/calendar', () => ({
  refreshCalendarView: vi.fn(),
}));

vi.mock('../../calendar/hooks/useCalendarData', () => ({
  invalidateCalendarCache: vi.fn(),
}));

const refreshCalendarViewMock = vi.mocked(refreshCalendarView);
const invalidateCalendarCacheMock = vi.mocked(invalidateCalendarCache);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('CalendarRefreshCard', () => {
  it('refreshes the calendar view and invalidates cache on success', async () => {
    refreshCalendarViewMock.mockResolvedValue({
      oldest: '2026-01-01',
      newest: '2026-04-27',
      rebuiltEntryCount: 3,
    });

    render(<CalendarRefreshCard apiBaseUrl="" />);

    fireEvent.click(screen.getByRole('button', { name: /przegeneruj widok kalendarza/i }));

    await waitFor(() => {
      expect(refreshCalendarViewMock).toHaveBeenCalledWith('');
    });
    expect(invalidateCalendarCacheMock).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/widok kalendarza zostal przegenerowany/i)).toBeInTheDocument();
  });

  it('shows an error message when refresh fails', async () => {
    refreshCalendarViewMock.mockRejectedValue(new Error('POST /api/calendar/refresh failed: 500'));

    render(<CalendarRefreshCard apiBaseUrl="" />);

    fireEvent.click(screen.getByRole('button', { name: /przegeneruj widok kalendarza/i }));

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('POST /api/calendar/refresh failed: 500');
    });
    expect(invalidateCalendarCacheMock).not.toHaveBeenCalled();
  });

  it('redirects to home when refresh returns 401', async () => {
    const location = window.location;
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...location, href: '/settings' },
    });
    refreshCalendarViewMock.mockRejectedValue(new AuthenticationError());

    render(<CalendarRefreshCard apiBaseUrl="" />);

    fireEvent.click(screen.getByRole('button', { name: /przegeneruj widok kalendarza/i }));

    await waitFor(() => {
      expect(window.location.href).toBe('/');
    });
    expect(invalidateCalendarCacheMock).not.toHaveBeenCalled();
    Object.defineProperty(window, 'location', { configurable: true, value: location });
  });
});
