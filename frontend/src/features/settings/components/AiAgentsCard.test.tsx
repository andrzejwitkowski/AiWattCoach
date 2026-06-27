import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { testAiAgentsConnection, updateAiAgents } from '../api/settings';
import { buildTestSettings } from '../mockData';
import { AiAgentsCard } from './AiAgentsCard';

vi.mock('../api/settings', () => ({
  updateAiAgents: vi.fn(),
  testAiAgentsConnection: vi.fn(),
}));

const updateAiAgentsMock = vi.mocked(updateAiAgents);
const testAiAgentsConnectionMock = vi.mocked(testAiAgentsConnection);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('AiAgentsCard', () => {
  it('shows persisted provider and model values', () => {
    render(<AiAgentsCard settings={buildTestSettings()} apiBaseUrl="" onSave={() => {}} />);

    expect(screen.getByLabelText(/active provider/i)).toHaveValue('openrouter');
    expect(screen.getByLabelText(/^Model$/i)).toHaveValue('openai/gpt-4o-mini');
    expect(screen.getByRole('button', { name: 'openai/gpt-5' })).toBeInTheDocument();
  });

  it('tests current values and omits unchanged masked provider key', async () => {
    testAiAgentsConnectionMock.mockResolvedValue({
      connected: true,
      message: 'Connection successful.',
      usedSavedApiKey: true,
      usedSavedProvider: false,
      usedSavedModel: false,
    });

    render(<AiAgentsCard settings={buildTestSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/^Model$/i), {
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
    updateAiAgentsMock.mockResolvedValue(buildTestSettings());
    const onSave = vi.fn();

    render(<AiAgentsCard settings={buildTestSettings()} apiBaseUrl="" onSave={onSave} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'openrouter' },
    });
    fireEvent.change(screen.getByLabelText(/^Model$/i), {
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

  it('saves deepseek provider, model, and key', async () => {
    updateAiAgentsMock.mockResolvedValue(buildTestSettings());
    const onSave = vi.fn();

    render(<AiAgentsCard settings={buildTestSettings({ aiAgents: { selectedProvider: null, selectedModel: null } })} apiBaseUrl="" onSave={onSave} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'deepseek' },
    });
    fireEvent.change(screen.getByLabelText(/^Model$/i), {
      target: { value: 'deepseek-v4-pro' },
    });
    fireEvent.change(screen.getByLabelText(/deepseek api key/i), {
      target: { value: 'sk-ds-new-key' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^save ai config$/i }));

    await waitFor(() => {
      expect(updateAiAgentsMock).toHaveBeenCalledWith('', {
        deepseekApiKey: 'sk-ds-new-key',
        selectedProvider: 'deepseek',
        selectedModel: 'deepseek-v4-pro',
      });
    });
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it('saves z.ai provider, model, and key', async () => {
    updateAiAgentsMock.mockResolvedValue(buildTestSettings());
    const onSave = vi.fn();

    render(<AiAgentsCard settings={buildTestSettings({ aiAgents: { selectedProvider: null, selectedModel: null } })} apiBaseUrl="" onSave={onSave} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'zai' },
    });
    fireEvent.change(screen.getByLabelText(/^Model$/i), {
      target: { value: 'glm-5.2' },
    });
    fireEvent.change(screen.getByLabelText(/z\.ai api key/i), {
      target: { value: 'sk-zai-new-key' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^save ai config$/i }));

    await waitFor(() => {
      expect(updateAiAgentsMock).toHaveBeenCalledWith('', {
        zaiApiKey: 'sk-zai-new-key',
        selectedProvider: 'zai',
        selectedModel: 'glm-5.2',
      });
    });
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it('clears plaintext api key fields after a successful save', async () => {
    updateAiAgentsMock.mockResolvedValue(buildTestSettings());

    render(<AiAgentsCard settings={buildTestSettings()} apiBaseUrl="" onSave={() => {}} />);

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
    updateAiAgentsMock.mockResolvedValue(
      buildTestSettings({ aiAgents: { selectedProvider: null, selectedModel: null } }),
    );

    render(<AiAgentsCard settings={buildTestSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: '' },
    });
    fireEvent.change(screen.getByLabelText(/^Model$/i), {
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

    render(<AiAgentsCard settings={buildTestSettings()} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: /^test connection$/i }));
    expect(screen.getByText(/testing the current visible ai draft/i)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/^Model$/i), {
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
    render(<AiAgentsCard settings={buildTestSettings({ aiAgents: { selectedProvider: null, selectedModel: null } })} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'gemini' },
    });

    expect(screen.getByLabelText(/^Model$/i)).toHaveValue('gemini-3-flash-preview');
  });

  it('autofills deepseek model when provider switches to deepseek', () => {
    render(<AiAgentsCard settings={buildTestSettings({ aiAgents: { selectedProvider: null, selectedModel: null } })} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'deepseek' },
    });

    expect(screen.getByLabelText(/^Model$/i)).toHaveValue('deepseek-v4-flash');
  });

  it('shows higher-end suggested models for each provider', () => {
    render(<AiAgentsCard settings={buildTestSettings({ aiAgents: { selectedProvider: 'openai', selectedModel: 'gpt-5' } })} apiBaseUrl="" onSave={() => {}} />);

    expect(screen.getByRole('button', { name: 'gpt-5' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'gpt-5.4' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'o4-mini' })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'gemini' },
    });

    expect(screen.getByRole('button', { name: 'gemini-3-flash-preview' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'gemini-2.5-flash' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'gemini-2.5-pro' })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'openrouter' },
    });

    expect(screen.getByRole('button', { name: 'openai/gpt-5' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'google/gemini-3-flash-preview' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'anthropic/claude-sonnet-4.5' })).toBeInTheDocument();
  });

  it('shows inline validation and disables actions when provider config is incomplete', () => {
    render(<AiAgentsCard settings={buildTestSettings({ aiAgents: { selectedProvider: null, selectedModel: null } })} apiBaseUrl="" onSave={() => {}} />);

    fireEvent.change(screen.getByLabelText(/active provider/i), {
      target: { value: 'openai' },
    });
    fireEvent.change(screen.getByLabelText(/^Model$/i), {
      target: { value: '' },
    });

    expect(screen.getByText(/choose a model for the selected provider/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^test connection$/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /^save ai config$/i })).toBeDisabled();
  });

  it('de-emphasizes irrelevant provider key fields', () => {
    render(<AiAgentsCard settings={buildTestSettings()} apiBaseUrl="" onSave={() => {}} />);

    const openaiInput = screen.getByLabelText(/openai api key/i);
    const openrouterInput = screen.getByLabelText(/openrouter api key/i);

    expect(openrouterInput.parentElement?.parentElement).toHaveClass('opacity-100');
    expect(openaiInput.parentElement?.parentElement).toHaveClass('opacity-60');
  });

  it('emphasizes deepseek key field when deepseek is selected', () => {
    render(<AiAgentsCard settings={buildTestSettings({ aiAgents: { selectedProvider: 'deepseek', selectedModel: 'deepseek-v4-flash' } })} apiBaseUrl="" onSave={() => {}} />);

    const deepseekInput = screen.getByLabelText(/deepseek api key/i);
    const openrouterInput = screen.getByLabelText(/openrouter api key/i);

    expect(deepseekInput.parentElement?.parentElement).toHaveClass('opacity-100');
    expect(openrouterInput.parentElement?.parentElement).toHaveClass('opacity-60');
  });

  it('does not claim an inactive provider key is saved when none exists', () => {
    render(
      <AiAgentsCard
        settings={buildTestSettings({
          aiAgents: {
            geminiApiKey: null,
            geminiApiKeySet: false,
            selectedProvider: 'openai',
            selectedModel: 'gpt-5',
          },
        })}
        apiBaseUrl=""
        onSave={() => {}}
      />,
    );

    expect(screen.getByText('Used by the active provider.')).toBeInTheDocument();
    expect(screen.getAllByText('Saved for quick provider switching.')).toHaveLength(1);
    expect(screen.queryAllByText('Optional unless you switch to this provider.')).toHaveLength(2);
  });
});
