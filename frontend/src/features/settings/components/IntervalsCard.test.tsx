import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { IntervalsCard } from './IntervalsCard';
import type { TestIntervalsConnectionResponse, UserSettingsResponse } from '../types';
import { testIntervalsConnection, updateIntervals } from '../api/settings';

vi.mock('../api/settings', () => ({
  updateIntervals: vi.fn(),
  testIntervalsConnection: vi.fn(),
}));

const updateIntervalsMock = vi.mocked(updateIntervals);
const testIntervalsConnectionMock = vi.mocked(testIntervalsConnection);
type TestResolver = (value: TestIntervalsConnectionResponse) => void;

function buildSettings(overrides?: Partial<UserSettingsResponse['intervals']>): UserSettingsResponse {
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
      apiKey: '***...1234',
      apiKeySet: true,
      athleteId: 'athlete-123',
      connected: false,
      ...overrides,
    },
    options: {
      analyzeWithoutHeartRate: false,
    },
    availability: {
      configured: true,
      days: [
        { weekday: 'mon', available: true, maxDurationMinutes: 60 },
        { weekday: 'tue', available: false, maxDurationMinutes: null },
        { weekday: 'wed', available: true, maxDurationMinutes: 90 },
        { weekday: 'thu', available: false, maxDurationMinutes: null },
        { weekday: 'fri', available: true, maxDurationMinutes: 120 },
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

  it('treats saved raw api key draft as clean after parent refreshes with masked value', async () => {
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
        settings={buildSettings({
          apiKey: '***...9999',
          apiKeySet: true,
          athleteId: 'athlete-999',
          connected: true,
        })}
        apiBaseUrl=""
        onSave={() => {}}
      />,
    );

    expect(screen.getByRole('button', { name: /^connect intervals$/i })).toBeDisabled();
  });

  it('sends explicit clears when saved intervals fields are blanked', async () => {
    updateIntervalsMock.mockResolvedValue(
      buildSettings({ apiKey: null, apiKeySet: false, athleteId: null, connected: false }),
    );

    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/api key/i), { target: { value: '' } });
    fireEvent.change(screen.getByLabelText(/athlete id/i), { target: { value: '   ' } });
    fireEvent.click(screen.getByRole('button', { name: /^connect intervals$/i }));

    await waitFor(() => {
      expect(updateIntervalsMock).toHaveBeenCalledWith('', {
        apiKey: null,
        athleteId: null,
      });
    });
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

  it('omits an unchanged athlete id when testing only a new api key draft', async () => {
    testIntervalsConnectionMock.mockResolvedValue({
      connected: true,
      message: 'Connection successful.',
      usedSavedApiKey: false,
      usedSavedAthleteId: true,
      persistedStatusUpdated: false,
    });

    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/api key/i), { target: { value: 'next-api-key' } });
    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));

    await waitFor(() => {
      expect(testIntervalsConnectionMock).toHaveBeenCalledWith('', {
        apiKey: 'next-api-key',
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

  it('refreshes settings after a successful test that persists connection state', async () => {
    testIntervalsConnectionMock.mockResolvedValue({
      connected: true,
      message: 'Connection successful.',
      usedSavedApiKey: false,
      usedSavedAthleteId: false,
      persistedStatusUpdated: true,
    });

    const onSave = vi.fn();
    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={onSave} />);

    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));

    await waitFor(() => {
      expect(onSave).toHaveBeenCalledTimes(1);
    });
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

  it('re-tests saved credentials for a disconnected user without editing fields', async () => {
    testIntervalsConnectionMock.mockResolvedValue({
      connected: true,
      message: 'Connection successful.',
      usedSavedApiKey: true,
      usedSavedAthleteId: true,
      persistedStatusUpdated: true,
    });

    const onSave = vi.fn();

    render(
      <IntervalsCard
        settings={buildSettings({ connected: false, apiKeySet: true, athleteId: 'athlete-123' })}
        apiBaseUrl=""
        onSave={onSave}
      />,
    );

    const connectButton = screen.getByRole('button', { name: /^connect intervals$/i });
    expect(connectButton).toBeEnabled();

    fireEvent.click(connectButton);

    await waitFor(() => {
      expect(testIntervalsConnectionMock).toHaveBeenCalledWith('', {});
    });

    expect(updateIntervalsMock).not.toHaveBeenCalled();
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it('does not allow reconnecting when saved credentials are incomplete', () => {
    render(
      <IntervalsCard
        settings={buildSettings({ connected: false, apiKeySet: true, athleteId: null })}
        apiBaseUrl=""
        onSave={() => {}}
      />,
    );

    expect(screen.getByRole('button', { name: /^connect intervals$/i })).toBeDisabled();
  });

  it('ignores stale in-flight test results after the draft changes', async () => {
    let resolveTest: TestResolver | undefined;

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
    expect(screen.getByRole('button', { name: /^test connection$/i })).not.toBeDisabled();

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

  it('refreshes settings for stale persisted test results without showing stale status', async () => {
    let resolveTest: TestResolver | undefined;

    testIntervalsConnectionMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTest = resolve;
        }),
    );

    const onSave = vi.fn();
    render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={onSave} />);

    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));
    fireEvent.change(screen.getByLabelText(/athlete id/i), { target: { value: 'athlete-555' } });

    await act(async () => {
      resolveTest?.({
        connected: true,
        message: 'Connection successful.',
        usedSavedApiKey: false,
        usedSavedAthleteId: false,
        persistedStatusUpdated: true,
      });
      await Promise.resolve();
    });

    expect(onSave).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(/^OK$/)).not.toBeInTheDocument();
    expect(screen.queryByText(/connection successful/i)).not.toBeInTheDocument();
  });

  it('keeps only later field edits dirty after a stale persisted api key test refresh', async () => {
    let resolveTest: TestResolver | undefined;

    testIntervalsConnectionMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTest = resolve;
        }),
    );

    const { rerender } = render(<IntervalsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/api key/i), { target: { value: 'next-api-key' } });
    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));
    fireEvent.change(screen.getByLabelText(/athlete id/i), { target: { value: 'athlete-555' } });

    await act(async () => {
      resolveTest?.({
        connected: true,
        message: 'Connection successful.',
        usedSavedApiKey: false,
        usedSavedAthleteId: true,
        persistedStatusUpdated: true,
      });
      await Promise.resolve();
    });

    rerender(
      <IntervalsCard
        settings={buildSettings({
          apiKey: '***...9999',
          apiKeySet: true,
          athleteId: 'athlete-123',
          connected: true,
        })}
        apiBaseUrl=""
        onSave={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /^connect intervals$/i }));

    await waitFor(() => {
      expect(updateIntervalsMock).toHaveBeenCalledWith('', {
        athleteId: 'athlete-555',
      });
    });
  });
});
