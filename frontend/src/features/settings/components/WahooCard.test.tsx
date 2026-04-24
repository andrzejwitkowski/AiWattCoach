import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { WahooCard } from './WahooCard';
import type { UserSettingsResponse } from '../types';

const originalLocation = window.location;

function buildSettings(overrides?: Partial<UserSettingsResponse['wahoo']>): UserSettingsResponse {
  return {
    aiAgents: {
      openaiApiKey: null,
      openaiApiKeySet: false,
      geminiApiKey: null,
      geminiApiKeySet: false,
      openrouterApiKey: null,
      openrouterApiKeySet: false,
      selectedProvider: null,
      selectedModel: null,
    },
    intervals: {
      apiKey: null,
      apiKeySet: false,
      athleteId: null,
      connected: false,
    },
    wahoo: {
      available: true,
      accessToken: null,
      accessTokenSet: false,
      refreshTokenSet: false,
      expiresAtEpochSeconds: null,
      connected: false,
      ...overrides,
    },
    options: {
      analyzeWithoutHeartRate: false,
    },
    availability: {
      configured: false,
      days: [
        { weekday: 'mon', available: false, maxDurationMinutes: null },
        { weekday: 'tue', available: false, maxDurationMinutes: null },
        { weekday: 'wed', available: false, maxDurationMinutes: null },
        { weekday: 'thu', available: false, maxDurationMinutes: null },
        { weekday: 'fri', available: false, maxDurationMinutes: null },
        { weekday: 'sat', available: false, maxDurationMinutes: null },
        { weekday: 'sun', available: false, maxDurationMinutes: null },
      ],
    },
    cycling: {
      fullName: null,
      age: null,
      heightCm: null,
      weightKg: null,
      ftpWatts: null,
      hrMaxBpm: null,
      vo2Max: null,
      athletePrompt: null,
      medications: null,
      athleteNotes: null,
      lastZoneUpdateEpochSeconds: null,
    },
  };
}

describe('WahooCard', () => {
  beforeEach(() => {
    window.history.replaceState({}, '', '/settings?tab=integrations');
  });

  afterEach(() => {
    cleanup();
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: originalLocation,
    });
    vi.restoreAllMocks();
  });

  it('shows unavailable state when wahoo oauth is not configured', () => {
    render(<WahooCard settings={buildSettings({ available: false })} apiBaseUrl="" />);

    expect(screen.getByText(/not configured on this server yet/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /connect wahoo/i })).toBeDisabled();
  });

  it('shows connected status details when tokens are present', () => {
    render(
      <WahooCard
        settings={buildSettings({
          connected: true,
          accessToken: '***...1234',
          accessTokenSet: true,
          refreshTokenSet: true,
          expiresAtEpochSeconds: 1_800_000_000,
        })}
        apiBaseUrl=""
      />,
    );

    expect(screen.getByText(/connected/i)).toBeInTheDocument();
    expect(screen.getAllByText(/^saved$/i)).toHaveLength(2);
    expect(screen.getByText(/tokens are stored/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /reconnect wahoo/i })).toBeInTheDocument();
  });

  it('starts the wahoo connect flow and preserves the current settings deep link', () => {
    const assignMock = vi.fn();
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, assign: assignMock },
    });

    render(<WahooCard settings={buildSettings()} apiBaseUrl="" />);

    fireEvent.click(screen.getByRole('button', { name: /connect wahoo/i }));

    expect(assignMock).toHaveBeenCalledWith(
      '/api/auth/wahoo/start?returnTo=%2Fsettings%3Ftab%3Dintegrations',
    );
  });
});
