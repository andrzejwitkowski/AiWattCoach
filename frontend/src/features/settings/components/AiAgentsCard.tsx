import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiKeyInput } from './ApiKeyInput';
import type { UserSettingsResponse } from '../types';
import { updateAiAgents } from '../api/settings';

type AiAgentsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

export function AiAgentsCard({ settings, apiBaseUrl, onSave }: AiAgentsCardProps) {
  const { t } = useTranslation();
  const [openaiKey, setOpenaiKey] = useState('');
  const [geminiKey, setGeminiKey] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const aiAgents = settings.aiAgents;

  async function handleSave() {
    const trimmedOpenai = openaiKey.trim();
    const trimmedGemini = geminiKey.trim();
    if (!trimmedOpenai && !trimmedGemini) return;
    setIsSaving(true);
    setSaved(false);
    setSaveError(null);
    try {
      const req: Record<string, string> = {};
      if (trimmedOpenai) req.openaiApiKey = trimmedOpenai;
      if (trimmedGemini) req.geminiApiKey = trimmedGemini;
      await updateAiAgents(apiBaseUrl, req);
      setOpenaiKey('');
      setGeminiKey('');
      setSaved(true);
      onSave();
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Failed to save AI configuration');
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="rounded-[1.5rem] border border-white/10 bg-gradient-to-br from-slate-900/80 to-slate-800/60 p-6">
      <div className="mb-4 flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-cyan-500/15">
          <svg className="h-5 w-5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 002.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.456 2.456L21.75 6l-1.035.259a3.375 3.375 0 00-2.456 2.456z" />
          </svg>
        </div>
        <div>
          <h3 className="text-lg font-semibold text-white">{t('aiAgents.title')}</h3>
          <p className="text-xs text-slate-400">{t('aiAgents.subtitle')}</p>
        </div>
      </div>

      <p className="mb-5 text-sm leading-relaxed text-slate-400">
        {t('aiAgents.description')}
      </p>

      <div className="space-y-4">
        <ApiKeyInput
          id="openai-key"
          label={t('aiAgents.openaiKey')}
          placeholder="sk-..."
          isConfigured={aiAgents.openaiApiKeySet}
          value={openaiKey}
          onChange={setOpenaiKey}
          accentColor="cyan"
        />

        <ApiKeyInput
          id="gemini-key"
          label={t('aiAgents.geminiKey')}
          placeholder="AIza..."
          isConfigured={aiAgents.geminiApiKeySet}
          value={geminiKey}
          onChange={setGeminiKey}
          accentColor="cyan"
        />

        {saveError && (
          <div className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
            {saveError}
          </div>
        )}

        <button
          type="button"
          className="flex w-full items-center justify-center gap-2 rounded-xl bg-cyan-500 px-4 py-2.5 text-sm font-semibold text-slate-950 transition hover:bg-cyan-400 disabled:opacity-50"
          disabled={isSaving || (!openaiKey.trim() && !geminiKey.trim())}
          onClick={() => { void handleSave(); }}
        >
          {isSaving ? t('aiAgents.saving') : saved ? t('aiAgents.saved') : t('aiAgents.save')}
        </button>
      </div>
    </div>
  );
}
