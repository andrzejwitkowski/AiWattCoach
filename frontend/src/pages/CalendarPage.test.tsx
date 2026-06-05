import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import i18n from '../i18n';
import { useCalendarCoachChat } from '../features/calendar/hooks/useCalendarCoachChat';
import { CalendarPage } from './CalendarPage';

function setScreenWidth(width: number) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query === '(max-width: 767px)' ? width <= 767 : false,
    media: query,
    onchange: null,
    addListener: () => undefined,
    removeListener: () => undefined,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    dispatchEvent: () => false,
  })) as typeof window.matchMedia;
}

vi.mock('../features/calendar/components/CalendarGrid', () => ({
  CalendarGrid: ({ apiBaseUrl }: { apiBaseUrl: string }) => (
    <div data-testid="calendar-grid">Calendar grid {apiBaseUrl}</div>
  ),
}));

vi.mock('../features/calendar/hooks/useCalendarCoachChat', () => ({
  useCalendarCoachChat: vi.fn(() => ({
    conversation: null,
    messages: [],
    isLoading: false,
    isStartingNewConversation: false,
    isConnected: false,
    isCoachTyping: false,
    error: null,
    sendMessage: vi.fn().mockResolvedValue(true),
    startNewConversation: vi.fn().mockResolvedValue(true),
  })),
}));

function renderCalendarPage(apiBaseUrl: string) {
  return render(
    <MemoryRouter>
      <CalendarPage apiBaseUrl={apiBaseUrl} />
    </MemoryRouter>,
  );
}

describe('CalendarPage', () => {
  beforeEach(async () => {
    setScreenWidth(1280);
    vi.mocked(useCalendarCoachChat).mockReset();
    vi.mocked(useCalendarCoachChat).mockReturnValue({
      conversation: null,
      messages: [],
      isLoading: false,
      isStartingNewConversation: false,
      isConnected: false,
      isCoachTyping: false,
      isCoachThinking: false,
      error: null,
      sendMessage: vi.fn().mockResolvedValue(true),
      startNewConversation: vi.fn().mockResolvedValue(true),
    });
    await i18n.changeLanguage('en');
  });

  afterEach(async () => {
    cleanup();
    await i18n.changeLanguage('en');
  });

  it('renders a fixed AI Coach button above the calendar', () => {
    renderCalendarPage('/api');

    expect(screen.getByTestId('calendar-grid')).toBeInTheDocument();

    const openButton = screen.getByRole('button', { name: /open ai coach/i });

    expect(openButton.className).toContain('fixed');
    expect(openButton.className).toContain('bottom-4');
    expect(openButton.className).toContain('right-4');
  });

  it('keeps the mobile-safe coach button spacing on narrow screens', () => {
    setScreenWidth(390);

    renderCalendarPage('/api');

    const openButton = screen.getByRole('button', { name: /open ai coach/i });

    expect(openButton.className).toContain('safe-bottom-inset');
    expect(openButton.className).toContain('bottom-4');
  });

  it('opens and closes the calendar coach modal', async () => {
    const user = userEvent.setup();

    renderCalendarPage('/backend');

    await user.click(screen.getByRole('button', { name: /open ai coach/i }));

    expect(screen.getByRole('dialog')).toHaveAttribute('aria-modal', 'true');
    expect(screen.getByRole('heading', { name: /ai coach - calendar review/i })).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: /calendar coach message/i })).toBeEnabled();
    expect(screen.getByRole('button', { name: /close calendar coach/i })).toHaveFocus();

    await user.click(screen.getByRole('button', { name: /close calendar coach/i }));

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    });
    expect(screen.getByRole('button', { name: /open ai coach/i })).toHaveFocus();
  });

  it('traps focus inside the modal while tabbing', async () => {
    const user = userEvent.setup();

    renderCalendarPage('');

    await user.click(screen.getByRole('button', { name: /open ai coach/i }));

    const closeButton = screen.getByRole('button', { name: /close calendar coach/i });
    const newConversationButton = screen.getByRole('button', { name: /new conversation/i });
    const messageInput = screen.getByRole('textbox', { name: /calendar coach message/i });

    expect(closeButton).toHaveFocus();

    await user.tab();
    expect(messageInput).toHaveFocus();

    await user.tab();
    expect(newConversationButton).toHaveFocus();

    await user.tab();
    expect(closeButton).toHaveFocus();

    await user.tab({ shift: true });
    expect(newConversationButton).toHaveFocus();
  });

  it('passes apiBaseUrl into the calendar coach hook through modal rendering', async () => {
    const { useCalendarCoachChat } = await import('../features/calendar/hooks/useCalendarCoachChat');
    const user = userEvent.setup();

    renderCalendarPage('/backend');

    await user.click(screen.getByRole('button', { name: /open ai coach/i }));

    expect(vi.mocked(useCalendarCoachChat)).toHaveBeenCalledWith({
      isOpen: true,
      onPlannedWorkoutUpdated: expect.any(Function),
    });
  });
});
