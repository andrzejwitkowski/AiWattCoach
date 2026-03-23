import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { IntervalsCard } from './IntervalsCard';
import type { UserSettingsResponse } from '../types';
import { testIntervalsConnection, updateIntervals } from '../api/settings';

vi.mock('../api/settings', () => ({
  updateIntervals: vi.fn(),
  testIntervalsConnection: vi.fn(),
}));

const updateIntervalsMock = vi.mocked(updateIntervals);
const testIntervalsConnectionMock = vi.mocked(testIntervalsConnection);

function buildSettings(overrides?: Partial<UserSettingsResponse['intervals']>): UserSettingsResponse {
  return {
    aiAgents: {
      openaiApiKey: null,
      openaiApiKeySet: false,
      geminiApiKey: null,
      geminiApiKeySet: false,
    },
    intervals: {
      apiKey: '***...1234',
      apiKeySet: true,
      athleteId: 'athlete-123',
      connected: false,
      ...overrides,
    },
    options: {
      analyzeWithoutHeartRate: false,
    },
    cycling: {
      fullName: null,
      age: null,
      heightCm: null,
      weightKg: null,
      ftpWatts: null,
      hrMaxBpm: null,
      vo2Max: null,
      lastZoneUpdateEpochSeconds: null,
    },
  };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('IntervalsCard', () => {
  it('shows the persisted intervals values in the visible inputs', () => {
    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    expect(screen.getByLabelText(/api key/i)).toHaveValue('***...1234');
    expect(screen.getByLabelText(/athlete id/i)).toHaveValue('athlete-123');
  });

  it('keeps the visible draft values after saving', async () => {
    updateIntervalsMock.mockResolvedValue(buildSettings({ athleteId: 'athlete-999' }));

    const onSave = vi.fn();
    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={onSave} />);

    const apiKeyInput = screen.getByLabelText(/api key/i);
    const athleteIdInput = screen.getByLabelText(/athlete id/i);

    fireEvent.change(apiKeyInput, { target: { value: 'next-api-key' } });
    fireEvent.change(athleteIdInput, { target: { value: 'athlete-999' } });
    fireEvent.click(screen.getByRole('button', { name: /^connect intervals$/i }));

    await waitFor(() => {
      expect(updateIntervalsMock).toHaveBeenCalled();
    });

    expect(apiKeyInput).toHaveValue('next-api-key');
    expect(athleteIdInput).toHaveValue('athlete-999');
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it('preserves the user draft after the parent refreshes with masked saved values', async () => {
    updateIntervalsMock.mockResolvedValue(buildSettings({ athleteId: 'athlete-999' }));

    const { rerender } = render(
      <IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />,
    );

    fireEvent.change(screen.getByLabelText(/api key/i), { target: { value: 'next-api-key' } });
    fireEvent.change(screen.getByLabelText(/athlete id/i), { target: { value: 'athlete-999' } });
    fireEvent.click(screen.getByRole('button', { name: /^connect intervals$/i }));

    await waitFor(() => {
      expect(updateIntervalsMock).toHaveBeenCalled();
    });

    rerender(
      <IntervalsCard
        settings={buildSettings({ apiKey: '***...9999', athleteId: 'athlete-999', apiKeySet: true })}
        apiBaseUrl=""
        onSave={() => {}}
      />,
    );

    expect(screen.getByLabelText(/api key/i)).toHaveValue('next-api-key');
    expect(screen.getByLabelText(/athlete id/i)).toHaveValue('athlete-999');
  });

  it('tests the currently displayed values and omits an unchanged masked api key', async () => {
    testIntervalsConnectionMock.mockResolvedValue({
      connected: true,
      message: 'Connection successful.',
      usedSavedApiKey: true,
      usedSavedAthleteId: false,
      persistedStatusUpdated: false,
    });

    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/athlete id/i), { target: { value: 'athlete-777' } });
    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));

    await waitFor(() => {
      expect(testIntervalsConnectionMock).toHaveBeenCalledWith('', {
        athleteId: 'athlete-777',
      });
    });
  });

  it('shows an OK status when the connection test succeeds', async () => {
    testIntervalsConnectionMock.mockResolvedValue({
      connected: true,
      message: 'Connection successful.',
      usedSavedApiKey: false,
      usedSavedAthleteId: false,
      persistedStatusUpdated: false,
    });

    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));

    expect(await screen.findByText(/^OK$/)).toBeInTheDocument();
    expect(screen.getByText(/connection successful/i)).toBeInTheDocument();
  });

  it('shows a FAILED status when the connection test fails', async () => {
    testIntervalsConnectionMock.mockResolvedValue({
      connected: false,
      message: 'Invalid API key or athlete ID. Please check your credentials.',
      usedSavedApiKey: false,
      usedSavedAthleteId: false,
      persistedStatusUpdated: false,
    });

    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));

    expect(await screen.findByText(/^FAILED$/)).toBeInTheDocument();
    expect(screen.getByText(/invalid api key or athlete id/i)).toBeInTheDocument();
  });

  it('clears stale test feedback after the draft changes', async () => {
    testIntervalsConnectionMock.mockResolvedValue({
      connected: true,
      message: 'Connection successful.',
      usedSavedApiKey: false,
      usedSavedAthleteId: false,
      persistedStatusUpdated: false,
    });

    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));
    expect(await screen.findByText(/^OK$/)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/athlete id/i), { target: { value: 'athlete-555' } });

    await waitFor(() => {
      expect(screen.queryByText(/^OK$/)).not.toBeInTheDocument();
    });
  });

  it('ignores stale in-flight test results after the draft changes', async () => {
    let resolveTest:
      | ((value: {
          connected: boolean;
          message: string;
          usedSavedApiKey: boolean;
          usedSavedAthleteId: boolean;
          persistedStatusUpdated: boolean;
        }) => void)
      | undefined;

    testIntervalsConnectionMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTest = resolve;
        }),
    );

    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));
    expect(screen.getByText(/testing current intervals\.icu values/i)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/athlete id/i), { target: { value: 'athlete-555' } });
    expect(screen.queryByText(/testing current intervals\.icu values/i)).not.toBeInTheDocument();

    await act(async () => {
      resolveTest?.({
        connected: true,
        message: 'Connection successful.',
        usedSavedApiKey: false,
        usedSavedAthleteId: false,
        persistedStatusUpdated: false,
      });
      await Promise.resolve();
    });

    expect(screen.queryByText(/^OK$/)).not.toBeInTheDocument();
    expect(screen.queryByText(/connection successful/i)).not.toBeInTheDocument();
  });
});
