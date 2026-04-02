import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AiAgentsCard } from './AiAgentsCard';
import type { UserSettingsResponse } from '../types';
import { testAiAgentsConnection, updateAiAgents } from '../api/settings';

vi.mock('../api/settings', () => ({
  updateAiAgents: vi.fn(),
  testAiAgentsConnection: vi.fn(),
}));

const updateAiAgentsMock = vi.mocked(updateAiAgents);
const testAiAgentsConnectionMock = vi.mocked(testAiAgentsConnection);

function buildSettings(overrides?: Partial<UserSettingsResponse['aiAgents']>): UserSettingsResponse {
  return {
    aiAgents: {
      openaiApiKey: '***...1234',
      openaiApiKeySet: true,
      geminiApiKey: null,
      geminiApiKeySet: false,
      openrouterApiKey: '***...9999',
      openrouterApiKeySet: true,
      selectedProvider: 'openrouter',
      selectedModel: 'openai/gpt-4o-mini',
      ...overrides,
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

describe('AiAgentsCard', () => {
  it('shows persisted provider and model values', () => {
    render(<AiAgentsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    expect(screen.getByLabelText(/active provider/i)).toHaveValue('openrouter');
    expect(screen.getByLabelText(/model/i)).toHaveValue('openai/gpt-4o-mini');
    expect(screen.getByRole('button', { name: 'openai/gpt-4o-mini' })).toBeInTheDocument();
  });

  it('tests current values and omits unchanged masked provider key', async () => {
    testAiAgentsConnectionMock.mockResolvedValue({
      connected: true,
      message: 'Connection successful.',
      usedSavedApiKey: true,
      usedSavedProvider: false,
      usedSavedModel: false,
    });

    render(<AiAgentsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/model/i), {
      target: { value: 'anthropic/claude-3.5-sonnet' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));

    await waitFor(() => {
      expect(testAiAgentsConnectionMock).toHaveBeenCalledWith('', {
        selectedModel: 'anthropic/claude-3.5-sonnet',
      });
    });
    expect(screen.getByText(/used saved key for unchanged fields/i)).toBeInTheDocument();
  });

  it('saves provider, model, and openrouter key', async () => {
    updateAiAgentsMock.mockResolvedValue(buildSettings());
    const onSave = vi.fn();

    render(<AiAgentsCard settings={buildSettings()} apiBaseUrl="" onSave={onSave} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'openrouter' },
    });
    fireEvent.change(screen.getByLabelText(/model/i), {
      target: { value: 'openai/gpt-4.1-mini' },
    });
    fireEvent.change(screen.getByLabelText(/openrouter api key/i), {
      target: { value: 'or-new-key' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^save ai config$/i }));

    await waitFor(() => {
      expect(updateAiAgentsMock).toHaveBeenCalledWith('', {
        openrouterApiKey: 'or-new-key',
        selectedModel: 'openai/gpt-4.1-mini',
      });
    });
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it('clears plaintext api key fields after a successful save', async () => {
    updateAiAgentsMock.mockResolvedValue(buildSettings());

    render(<AiAgentsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    const openrouterKeyInput = screen.getByLabelText(/openrouter api key/i) as HTMLInputElement;
    fireEvent.change(openrouterKeyInput, {
      target: { value: 'or-new-key' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^save ai config$/i }));

    await waitFor(() => {
      expect(updateAiAgentsMock).toHaveBeenCalled();
    });

    expect(openrouterKeyInput.value).toBe('');
  });

  it('sends explicit provider and model clears on save', async () => {
    updateAiAgentsMock.mockResolvedValue(buildSettings({ selectedProvider: null, selectedModel: null }));

    render(<AiAgentsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: '' },
    });
    fireEvent.change(screen.getByLabelText(/model/i), {
      target: { value: '' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^save ai config$/i }));

    await waitFor(() => {
      expect(updateAiAgentsMock).toHaveBeenCalledWith('', {
        selectedProvider: '',
        selectedModel: '',
      });
    });
  });

  it('ignores stale test responses after the draft changes', async () => {
    let resolveTest:
      | ((value: {
          connected: boolean;
          message: string;
          usedSavedApiKey: boolean;
          usedSavedProvider: boolean;
          usedSavedModel: boolean;
        }) => void)
      | undefined;

    testAiAgentsConnectionMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTest = resolve;
        }),
    );

    render(<AiAgentsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));
    expect(screen.getByText(/testing the current visible ai draft/i)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/model/i), {
      target: { value: 'gpt-4o-mini' },
    });

    await act(async () => {
      resolveTest?.({
        connected: true,
        message: 'Connection successful.',
        usedSavedApiKey: false,
        usedSavedProvider: false,
        usedSavedModel: false,
      });
      await Promise.resolve();
    });

    expect(screen.queryByText(/^OK$/)).not.toBeInTheDocument();
    expect(screen.queryByText(/connection successful/i)).not.toBeInTheDocument();
  });

  it('autofills a recommended model when provider changes', () => {
    render(<AiAgentsCard settings={buildSettings({ selectedProvider: null, selectedModel: null })} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'gemini' },
    });

    expect(screen.getByLabelText(/model/i)).toHaveValue('gemini-2.5-flash');
  });

  it('shows inline validation and disables actions when provider config is incomplete', () => {
    render(<AiAgentsCard settings={buildSettings({ selectedProvider: null, selectedModel: null })} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'openai' },
    });
    fireEvent.change(screen.getByLabelText(/model/i), {
      target: { value: '' },
    });

    expect(screen.getByText(/choose a model for the selected provider/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^test connection$/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /^save ai config$/i })).toBeDisabled();
  });

  it('de-emphasizes irrelevant provider key fields', () => {
    render(<AiAgentsCard settings={buildSettings()} apiBaseUrl="" onSave={() => {}} />);

    const openaiInput = screen.getByLabelText(/openai api key/i);
    const openrouterInput = screen.getByLabelText(/openrouter api key/i);

    expect(openrouterInput.parentElement?.parentElement).toHaveClass('opacity-100');
    expect(openaiInput.parentElement?.parentElement).toHaveClass('opacity-60');
  });
});
