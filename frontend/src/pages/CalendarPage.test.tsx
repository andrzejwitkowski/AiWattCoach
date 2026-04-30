import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import i18n from '../i18n';
import { CalendarPage } from './CalendarPage';

vi.mock('../features/calendar/components/CalendarGrid', () => ({
  CalendarGrid: ({ apiBaseUrl }: { apiBaseUrl: string }) => (
    <div data-testid="calendar-grid">Calendar grid {apiBaseUrl}</div>
  ),
}));

describe('CalendarPage', () => {
  afterEach(async () => {
    cleanup();
    await i18n.changeLanguage('en');
  });

  it('renders a fixed AI Coach button above the calendar', () => {
    render(<CalendarPage apiBaseUrl="" />);

    expect(screen.getByTestId('calendar-grid')).toBeInTheDocument();

    const openButton = screen.getByRole('button', { name: /open ai coach preview/i });

    expect(openButton.className).toContain('fixed');
    expect(openButton.className).toContain('bottom-4');
    expect(openButton.className).toContain('right-4');
  });

  it('opens and closes the calendar coach modal', async () => {
    const user = userEvent.setup();

    render(<CalendarPage apiBaseUrl="" />);

    await user.click(screen.getByRole('button', { name: /open ai coach preview/i }));

    expect(screen.getByRole('dialog')).toHaveAttribute('aria-modal', 'true');
    expect(screen.getByRole('heading', { name: /ai coach - calendar review/i })).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: /calendar coach message preview/i })).toBeDisabled();

    await user.click(screen.getByRole('button', { name: /close calendar coach preview/i }));

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('closes the modal on escape', async () => {
    const user = userEvent.setup();

    render(<CalendarPage apiBaseUrl="" />);

    await user.click(screen.getByRole('button', { name: /open ai coach preview/i }));
    expect(screen.getByRole('dialog')).toBeInTheDocument();

    await user.keyboard('{Escape}');

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});
