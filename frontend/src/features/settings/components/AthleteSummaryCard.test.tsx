import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { generateAthleteSummary, loadAthleteSummary } from '../api/athleteSummary';
import { buildTestSettings } from '../mockData';
import type { UserSettingsResponse } from '../types';
import { AthleteSummaryCard } from './AthleteSummaryCard';

vi.mock('../api/athleteSummary', () => ({
  loadAthleteSummary: vi.fn(),
  generateAthleteSummary: vi.fn(),
}));

const loadAthleteSummaryMock = vi.mocked(loadAthleteSummary);
const generateAthleteSummaryMock = vi.mocked(generateAthleteSummary);

type SettingsOverrides = {
  aiAgents?: Partial<UserSettingsResponse['aiAgents']>;
  intervals?: Partial<UserSettingsResponse['intervals']>;
  wahoo?: Partial<UserSettingsResponse['wahoo']>;
  options?: Partial<UserSettingsResponse['options']>;
  availability?: Partial<UserSettingsResponse['availability']>;
  cycling?: Partial<UserSettingsResponse['cycling']>;
};

function buildSettings(overrides?: SettingsOverrides): UserSettingsResponse {
  return buildTestSettings({
    aiAgents: {
      openaiApiKey: null,
      openaiApiKeySet: false,
      geminiApiKey: null,
      geminiApiKeySet: false,
      openrouterApiKey: '***...9999',
      openrouterApiKeySet: true,
      deepseekApiKey: null,
      deepseekApiKeySet: false,
      selectedProvider: 'openrouter',
      selectedModel: 'google/gemini-3-flash-preview',
      ...overrides?.aiAgents,
    },
    intervals: {
      apiKey: '***...1234',
      apiKeySet: true,
      athleteId: 'i248035',
      connected: true,
      ...overrides?.intervals,
    },
    wahoo: overrides?.wahoo,
    options: overrides?.options,
    availability: overrides?.availability,
    cycling: overrides?.cycling,
  });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('AthleteSummaryCard', () => {
  it('shows fetched summary in a read-only textbox', async () => {
    loadAthleteSummaryMock.mockResolvedValue({
      exists: true,
      stale: false,
      summaryText: 'Strong aerobic durability with weak repeatability above threshold.',
      generatedAtEpochSeconds: 1_700_000_000,
      updatedAtEpochSeconds: 1_700_000_000,
    });

    render(<AthleteSummaryCard settings={buildSettings()} apiBaseUrl="" />);

    await waitFor(() => {
      expect(screen.getByLabelText(/athlete summary/i)).toHaveValue(
        'Strong aerobic durability with weak repeatability above threshold.',
      );
    });
    expect(screen.getByLabelText(/athlete summary/i)).toHaveAttribute('readonly');
  });

  it('shows create button only when required configs are available', async () => {
    loadAthleteSummaryMock.mockResolvedValue({ exists: false, stale: true, summaryText: null });

    render(
      <AthleteSummaryCard
        settings={buildSettings({ intervals: { apiKeySet: false, athleteId: null, connected: false } })}
        apiBaseUrl=""
      />,
    );

    await waitFor(() => {
      expect(loadAthleteSummaryMock).toHaveBeenCalled();
    });
    expect(screen.getByRole('button', { name: /create athlete summary/i })).toBeDisabled();
  });

  it('requires an api key for the selected provider', async () => {
    loadAthleteSummaryMock.mockResolvedValue({ exists: false, stale: true, summaryText: null });

    render(
      <AthleteSummaryCard
        settings={buildSettings({
          aiAgents: {
            selectedProvider: 'openai',
            selectedModel: 'gpt-4o-mini',
            openaiApiKeySet: false,
            geminiApiKeySet: false,
            openrouterApiKeySet: true,
          },
        })}
        apiBaseUrl=""
      />,
    );

    await waitFor(() => {
      expect(loadAthleteSummaryMock).toHaveBeenCalled();
    });

    expect(screen.getByRole('button', { name: /create athlete summary/i })).toBeDisabled();
  });

  it('generates and updates the summary textbox', async () => {
    loadAthleteSummaryMock
      .mockResolvedValueOnce({ exists: false, stale: true, summaryText: null })
      .mockResolvedValueOnce({
        exists: true,
        stale: false,
        summaryText: 'Well-rounded athlete with strong diesel engine and improving tempo control.',
        generatedAtEpochSeconds: 1_700_000_000,
        updatedAtEpochSeconds: 1_700_000_000,
      });
    generateAthleteSummaryMock.mockResolvedValue({
      exists: true,
      stale: false,
      summaryText: 'Well-rounded athlete with strong diesel engine and improving tempo control.',
      generatedAtEpochSeconds: 1_700_000_000,
      updatedAtEpochSeconds: 1_700_000_000,
    });

    render(<AthleteSummaryCard settings={buildSettings()} apiBaseUrl="" />);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /create athlete summary/i })).toBeEnabled();
    });

    fireEvent.click(screen.getByRole('button', { name: /create athlete summary/i }));

    await waitFor(() => {
      expect(generateAthleteSummaryMock).toHaveBeenCalledWith('');
    });

    await waitFor(() => {
      expect(screen.getByLabelText(/athlete summary/i)).toHaveValue(
        'Well-rounded athlete with strong diesel engine and improving tempo control.',
      );
    });
  });

  it('reloads the latest summary when generate returns an error after work was submitted', async () => {
    loadAthleteSummaryMock
      .mockResolvedValueOnce({ exists: true, stale: false, summaryText: 'Old summary' })
      .mockResolvedValueOnce({
        exists: true,
        stale: false,
        summaryText: 'Refreshed summary from follow-up load',
        generatedAtEpochSeconds: 1_700_000_100,
        updatedAtEpochSeconds: 1_700_000_100,
      });
    generateAthleteSummaryMock.mockRejectedValue(new Error('POST /api/athlete-summary/generate failed: 503'));

    render(<AthleteSummaryCard settings={buildSettings()} apiBaseUrl="" />);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /refresh athlete summary/i })).toBeEnabled();
    });

    fireEvent.click(screen.getByRole('button', { name: /refresh athlete summary/i }));

    await waitFor(() => {
      expect(generateAthleteSummaryMock).toHaveBeenCalledWith('');
    });

    await waitFor(() => {
      expect(loadAthleteSummaryMock).toHaveBeenCalledTimes(2);
    });

    await waitFor(() => {
      expect(screen.getByLabelText(/athlete summary/i)).toHaveValue(
        'Refreshed summary from follow-up load',
      );
    });
  });
});
