import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { LandingPage } from './LandingPage';

describe('LandingPage', () => {
  it('renders all login UI elements', () => {
    const onLogin = vi.fn();
    render(<LandingPage onLogin={onLogin} onJoinWhitelist={vi.fn()} />);

    expect(screen.getByRole('button', { name: /sign in with google/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 1 })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /get started/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /join whitelist/i })).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/you@example.com/i)).toBeInTheDocument();
    expect(screen.queryByText(/dev auth enabled/i)).not.toBeInTheDocument();
  });

  it('shows a visible hint when dev auth is enabled', () => {
    render(<LandingPage onLogin={vi.fn()} onJoinWhitelist={vi.fn()} devAuthEnabled />);

    expect(screen.getByText(/dev auth enabled/i)).toBeInTheDocument();
    expect(
      screen.getByText(/google sign-in uses the configured mock athlete/i)
    ).toBeInTheDocument();
  });

  it('calls onLogin when Google sign-in or Get Started buttons are clicked', () => {
    const onLogin = vi.fn();
    const { container } = render(<LandingPage onLogin={onLogin} onJoinWhitelist={vi.fn()} />);

    const buttons = container.querySelectorAll('button');
    let googleButton: Element | null = null;
    let getStartedButton: Element | null = null;

    for (const btn of buttons) {
      if (btn.textContent?.match(/Sign in with Google/)) {
        googleButton = btn;
      }
      if (btn.textContent?.match(/Get Started/)) {
        getStartedButton = btn;
      }
    }

    if (googleButton) fireEvent.click(googleButton);
    expect(onLogin).toHaveBeenCalledTimes(1);

    if (getStartedButton) fireEvent.click(getStartedButton);
    expect(onLogin).toHaveBeenCalledTimes(2);
  });

  it('renders auth and whitelist messages when provided', () => {
    render(
      <LandingPage
        onLogin={vi.fn()}
        onJoinWhitelist={vi.fn()}
        authMessage="Nie jestes jeszcze przyjety"
        whitelistMessage="Dodalismy Cie do whitelisty"
      />
    );

    expect(screen.getByText(/nie jestes jeszcze przyjety/i)).toBeInTheDocument();
    expect(screen.getByText(/dodalismy cie do whitelisty/i)).toBeInTheDocument();
  });
});
