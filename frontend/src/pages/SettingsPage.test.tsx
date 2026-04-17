import { act, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { SettingsPage } from './SettingsPage';
import { SettingsProvider } from '../features/settings/context/SettingsContext';
import type { UserSettingsResponse } from '../features/settings/types';

vi.mock('../features/settings/api/settings', () => ({
  loadSettings: vi.fn(),
}));

vi.mock('../features/settings/components/AiAgentsCard', () => ({
  AiAgentsCard: () => <div>ai-agents-card</div>,
}));

vi.mock('../features/settings/components/AvailabilityCard', () => ({
  AvailabilityCard: () => <div>availability-card</div>,
}));

vi.mock('../features/settings/components/AthleteSummaryCard', () => ({
  AthleteSummaryCard: () => <div>athlete-summary-card</div>,
}));

vi.mock('../features/settings/components/CyclingSettingsCard', () => ({
  CyclingSettingsCard: () => <div>cycling-settings-card</div>,
}));

vi.mock('../features/settings/components/OptionsCard', () => ({
  OptionsCard: () => <div>options-card</div>,
}));

vi.mock('../features/settings/components/IntervalsCard', () => ({
  IntervalsCard: vi.fn(() => <button type="button">Test Connection</button>),
}));

const { loadSettings } = await import('../features/settings/api/settings');
const { IntervalsCard } = await import('../features/settings/components/IntervalsCard');

const loadSettingsMock = vi.mocked(loadSettings);
const intervalsCardMock = vi.mocked(IntervalsCard);

const settingsFixture: UserSettingsResponse = {
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

afterEach(() => {
  vi.clearAllMocks();
});

describe('SettingsPage', () => {
  it('keeps cards visible during background refresh after save', async () => {
    let resolveBackgroundRefresh: ((value: UserSettingsResponse) => void) | undefined;

    loadSettingsMock
      .mockResolvedValueOnce(settingsFixture)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveBackgroundRefresh = resolve;
          }),
      );

    render(
      <SettingsProvider apiBaseUrl="">
        <SettingsPage apiBaseUrl="" />
      </SettingsProvider>,
    );

    expect(await screen.findByText('ai-agents-card')).toBeInTheDocument();

    const intervalsProps = intervalsCardMock.mock.calls.at(-1)?.[0];
    expect(intervalsProps).toBeDefined();

    await act(async () => {
      intervalsProps?.onSave();
      await Promise.resolve();
    });

    expect(document.querySelector('.animate-spin')).toBeNull();
    expect(screen.getByText('ai-agents-card')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^test connection$/i })).toBeInTheDocument();

    await act(async () => {
      resolveBackgroundRefresh?.(settingsFixture);
      await Promise.resolve();
    });

    expect(document.querySelector('.animate-spin')).toBeNull();
    expect(screen.getByText('ai-agents-card')).toBeInTheDocument();
    expect(intervalsCardMock).toHaveBeenCalled();
  });
});
