import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { AvailabilityCard } from './AvailabilityCard';
import type { UserSettingsResponse } from '../types';
import { updateAvailability } from '../api/settings';

vi.mock('../api/settings', () => ({
  updateAvailability: vi.fn(),
}));

const updateAvailabilityMock = vi.mocked(updateAvailability);

function buildSettings(): UserSettingsResponse {
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

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('AvailabilityCard', () => {
  it('saves explicit weekday availability with fixed duration steps', async () => {
    updateAvailabilityMock.mockResolvedValue(buildSettings());

    render(<AvailabilityCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.click(screen.getByRole('switch', { name: /monday availability/i }));
    fireEvent.change(screen.getByRole('combobox', { name: /monday max duration/i }), {
      target: { value: '90' },
    });
    fireEvent.click(screen.getByRole('button', { name: /save availability/i }));

    await waitFor(() => {
      expect(updateAvailabilityMock).toHaveBeenCalledWith('', {
        days: [
          { weekday: 'mon', available: true, maxDurationMinutes: 90 },
          { weekday: 'tue', available: false, maxDurationMinutes: null },
          { weekday: 'wed', available: false, maxDurationMinutes: null },
          { weekday: 'thu', available: false, maxDurationMinutes: null },
          { weekday: 'fri', available: false, maxDurationMinutes: null },
          { weekday: 'sat', available: false, maxDurationMinutes: null },
          { weekday: 'sun', available: false, maxDurationMinutes: null },
        ],
      });
    });
  });

  it('clears duration when a day is unavailable', async () => {
    updateAvailabilityMock.mockResolvedValue(buildSettings());
    const settings = buildSettings();
    settings.availability = {
      configured: true,
      days: [
        { weekday: 'mon', available: true, maxDurationMinutes: 60 },
        { weekday: 'tue', available: false, maxDurationMinutes: null },
        { weekday: 'wed', available: false, maxDurationMinutes: null },
        { weekday: 'thu', available: false, maxDurationMinutes: null },
        { weekday: 'fri', available: false, maxDurationMinutes: null },
        { weekday: 'sat', available: false, maxDurationMinutes: null },
        { weekday: 'sun', available: false, maxDurationMinutes: null },
      ],
    };

    render(<AvailabilityCard settings={settings} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.click(screen.getByRole('switch', { name: /monday availability/i }));
    fireEvent.click(screen.getByRole('button', { name: /save availability/i }));

    await waitFor(() => {
      expect(updateAvailabilityMock).toHaveBeenCalledWith('', expect.objectContaining({
        days: expect.arrayContaining([
          expect.objectContaining({ weekday: 'mon', available: false, maxDurationMinutes: null }),
        ]),
      }));
    });
  });

  it('resyncs local draft when refreshed settings change', () => {
    const { rerender } = render(<AvailabilityCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    expect(screen.getByRole('switch', { name: /monday availability/i })).toHaveAttribute('aria-checked', 'false');

    const updatedSettings = buildSettings();
    updatedSettings.availability = {
      configured: true,
      days: [
        { weekday: 'mon', available: true, maxDurationMinutes: 120 },
        { weekday: 'tue', available: false, maxDurationMinutes: null },
        { weekday: 'wed', available: false, maxDurationMinutes: null },
        { weekday: 'thu', available: false, maxDurationMinutes: null },
        { weekday: 'fri', available: false, maxDurationMinutes: null },
        { weekday: 'sat', available: false, maxDurationMinutes: null },
        { weekday: 'sun', available: false, maxDurationMinutes: null },
      ],
    };

    rerender(<AvailabilityCard settings={updatedSettings} apiBaseUrl="" onSave={() => {}} />);

    expect(screen.getByRole('switch', { name: /monday availability/i })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('combobox', { name: /monday max duration/i })).toHaveValue('120');
  });

  it('preserves unsaved edits across unrelated settings refreshes', () => {
    const { rerender } = render(<AvailabilityCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.click(screen.getByRole('switch', { name: /monday availability/i }));
    fireEvent.change(screen.getByRole('combobox', { name: /monday max duration/i }), {
      target: { value: '150' },
    });

    const refreshedSettings = buildSettings();
    rerender(<AvailabilityCard settings={refreshedSettings} apiBaseUrl="" onSave={() => {}} />);

    expect(screen.getByRole('switch', { name: /monday availability/i })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('combobox', { name: /monday max duration/i })).toHaveValue('150');
  });

  it('exposes accessible labels for weekday controls', () => {
    render(<AvailabilityCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    expect(screen.getByRole('switch', { name: /monday availability/i })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: /monday max duration/i })).toBeInTheDocument();
  });

  it('announces save errors to assistive technology', async () => {
    updateAvailabilityMock.mockRejectedValue(new Error('Failed to save availability'));

    render(<AvailabilityCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: /save availability/i }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/failed to save availability/i);
  });

  it('passes updated settings to onSave after a successful save', async () => {
    const updatedSettings = buildSettings();
    updatedSettings.availability = {
      configured: true,
      days: [
        { weekday: 'mon', available: true, maxDurationMinutes: 90 },
        { weekday: 'tue', available: false, maxDurationMinutes: null },
        { weekday: 'wed', available: false, maxDurationMinutes: null },
        { weekday: 'thu', available: false, maxDurationMinutes: null },
        { weekday: 'fri', available: false, maxDurationMinutes: null },
        { weekday: 'sat', available: false, maxDurationMinutes: null },
        { weekday: 'sun', available: false, maxDurationMinutes: null },
      ],
    };
    updateAvailabilityMock.mockResolvedValue(updatedSettings);
    const onSave = vi.fn();

    render(<AvailabilityCard settings={buildSettings()} apiBaseUrl="" onSave={onSave} />);

    fireEvent.click(screen.getByRole('switch', { name: /monday availability/i }));
    fireEvent.change(screen.getByRole('combobox', { name: /monday max duration/i }), {
      target: { value: '90' },
    });
    fireEvent.click(screen.getByRole('button', { name: /save availability/i }));

    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith(updatedSettings);
    });
  });
});
