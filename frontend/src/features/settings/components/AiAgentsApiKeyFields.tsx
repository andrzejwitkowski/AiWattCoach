import { useState } from 'react';
import { Eye, EyeOff } from 'lucide-react';

import type { AiAgentsDraftState } from '../aiAgentsDraft';
import type { UserSettingsResponse } from '../types';

type AiAgentsApiKeyFieldsProps = {
  aiAgents: UserSettingsResponse['aiAgents'];
  draft: AiAgentsDraftState;
  onUpdate: (field: keyof AiAgentsDraftState, value: string) => void;
};

export function AiAgentsApiKeyFields({ aiAgents, draft, onUpdate }: AiAgentsApiKeyFieldsProps) {
  const [showOpenai, setShowOpenai] = useState(false);
  const [showGemini, setShowGemini] = useState(false);
  const [showOpenrouter, setShowOpenrouter] = useState(false);
  const [showDeepseek, setShowDeepseek] = useState(false);
  const [showZai, setShowZai] = useState(false);

  return (
    <div className="mt-6 space-y-4">
      <ApiKeyField
        id="openai-api-key"
        label="OpenAI API Key"
        placeholder={aiAgents.openaiApiKeySet ? 'Already configured' : 'sk-...'}
        value={draft.openaiApiKey}
        visible={showOpenai}
        configured={aiAgents.openaiApiKeySet}
        emphasized={!draft.selectedProvider || draft.selectedProvider === 'openai'}
        helperText={apiKeyHelperText(
          draft.selectedProvider,
          'openai',
          aiAgents.openaiApiKeySet || draft.openaiApiKey.trim().length > 0,
        )}
        onVisibilityChange={() => setShowOpenai((value) => !value)}
        onChange={(value) => onUpdate('openaiApiKey', value)}
      />
      <ApiKeyField
        id="gemini-api-key"
        label="Gemini API Key"
        placeholder={aiAgents.geminiApiKeySet ? 'Already configured' : 'AIza...'}
        value={draft.geminiApiKey}
        visible={showGemini}
        configured={aiAgents.geminiApiKeySet}
        emphasized={!draft.selectedProvider || draft.selectedProvider === 'gemini'}
        helperText={apiKeyHelperText(
          draft.selectedProvider,
          'gemini',
          aiAgents.geminiApiKeySet || draft.geminiApiKey.trim().length > 0,
        )}
        onVisibilityChange={() => setShowGemini((value) => !value)}
        onChange={(value) => onUpdate('geminiApiKey', value)}
      />
      <ApiKeyField
        id="openrouter-api-key"
        label="OpenRouter API Key"
        placeholder={aiAgents.openrouterApiKeySet ? 'Already configured' : 'sk-or-...'}
        value={draft.openrouterApiKey}
        visible={showOpenrouter}
        configured={aiAgents.openrouterApiKeySet}
        emphasized={!draft.selectedProvider || draft.selectedProvider === 'openrouter'}
        helperText={apiKeyHelperText(
          draft.selectedProvider,
          'openrouter',
          aiAgents.openrouterApiKeySet || draft.openrouterApiKey.trim().length > 0,
        )}
        onVisibilityChange={() => setShowOpenrouter((value) => !value)}
        onChange={(value) => onUpdate('openrouterApiKey', value)}
      />
      <ApiKeyField
        id="deepseek-api-key"
        label="DeepSeek API Key"
        placeholder={aiAgents.deepseekApiKeySet ? 'Already configured' : 'sk-...'}
        value={draft.deepseekApiKey}
        visible={showDeepseek}
        configured={aiAgents.deepseekApiKeySet}
        emphasized={!draft.selectedProvider || draft.selectedProvider === 'deepseek'}
        helperText={apiKeyHelperText(
          draft.selectedProvider,
          'deepseek',
          aiAgents.deepseekApiKeySet || draft.deepseekApiKey.trim().length > 0,
        )}
        onVisibilityChange={() => setShowDeepseek((value) => !value)}
        onChange={(value) => onUpdate('deepseekApiKey', value)}
      />
      <ApiKeyField
        id="zai-api-key"
        label="z.ai API Key"
        placeholder={aiAgents.zaiApiKeySet ? 'Already configured' : 'sk-...'}
        value={draft.zaiApiKey}
        visible={showZai}
        configured={aiAgents.zaiApiKeySet}
        emphasized={!draft.selectedProvider || draft.selectedProvider === 'zai'}
        helperText={apiKeyHelperText(
          draft.selectedProvider,
          'zai',
          aiAgents.zaiApiKeySet || draft.zaiApiKey.trim().length > 0,
        )}
        onVisibilityChange={() => setShowZai((value) => !value)}
        onChange={(value) => onUpdate('zaiApiKey', value)}
      />
    </div>
  );
}

function apiKeyHelperText(selectedProvider: string, provider: string, hasKey: boolean) {
  if (selectedProvider === provider) {
    return 'Used by the active provider.';
  }
  if (hasKey) {
    return 'Saved for quick provider switching.';
  }
  return 'Optional unless you switch to this provider.';
}

type ApiKeyFieldProps = {
  id: string;
  label: string;
  placeholder: string;
  value: string;
  visible: boolean;
  configured: boolean;
  emphasized: boolean;
  helperText: string;
  onVisibilityChange: () => void;
  onChange: (value: string) => void;
};

function ApiKeyField({
  id,
  label,
  placeholder,
  value,
  visible,
  configured,
  emphasized,
  helperText,
  onVisibilityChange,
  onChange,
}: ApiKeyFieldProps) {
  return (
    <div className={emphasized ? 'opacity-100' : 'opacity-60'}>
      <label htmlFor={id} className="mb-2 block text-xs uppercase tracking-widest text-slate-400">
        {label}
      </label>
      <div className="relative">
        <input
          id={id}
          className="w-full rounded-xl border border-white/10 bg-slate-900/60 px-4 py-3 pr-10 text-sm text-slate-200 placeholder:text-slate-600 focus:border-cyan-400/50 focus:outline-none"
          type={visible ? 'text' : 'password'}
          placeholder={placeholder}
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
        <button
          className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 transition hover:text-slate-200"
          onClick={onVisibilityChange}
          type="button"
          aria-label={visible ? 'Hide key' : 'Show key'}
        >
          {visible ? <EyeOff size={16} /> : <Eye size={16} />}
        </button>
      </div>
      <p className="mt-1.5 text-xs text-slate-400">{helperText}</p>
      {configured && <p className="mt-1 text-xs text-emerald-400">API key is configured</p>}
    </div>
  );
}
