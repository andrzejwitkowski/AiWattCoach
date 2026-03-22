import { useState } from 'react';
import type { UpdateAiAgentsRequest, UserSettingsResponse } from '../types';
import { updateAiAgents } from '../api/settings';

type AiAgentsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

export function AiAgentsCard({ settings, apiBaseUrl, onSave }: AiAgentsCardProps) {
  const [openaiKey, setOpenaiKey] = useState('');
  const [geminiKey, setGeminiKey] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const aiAgents = settings.aiAgents;

  async function handleSave() {
    setIsSaving(true);
    setSaved(false);
    try {
      const req: UpdateAiAgentsRequest = {};
      if (openaiKey) req.openaiApiKey = openaiKey;
      if (geminiKey) req.geminiApiKey = geminiKey;
      await updateAiAgents(apiBaseUrl, req);
      setOpenaiKey('');
      setGeminiKey('');
      setSaved(true);
      onSave();
    } catch {
      // handle error
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
          <h3 className="text-lg font-semibold text-white">AI Agents</h3>
          <p className="text-xs text-slate-400">Customize your intelligence</p>
        </div>
      </div>

      <p className="mb-5 text-sm leading-relaxed text-slate-400">
        Bring Your Own Key (BYOK). Enter your API keys to unlock advanced AI-powered training analysis.
      </p>

      <div className="space-y-4">
        <div>
          <label className="mb-1.5 block text-xs font-medium uppercase tracking-wider text-slate-400" htmlFor="openai-key">
            OpenAI API Key
          </label>
          <div className="relative">
            <input
              id="openai-key"
              type="password"
              className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm text-white placeholder-slate-500 focus:border-cyan-500/50 focus:outline-none focus:ring-1 focus:ring-cyan-500/30"
              placeholder={aiAgents.openaiApiKeySet ? '••••••••••••••••••••••' : 'sk-...'}
              value={openaiKey}
              onChange={(e) => setOpenaiKey(e.target.value)}
            />
            {aiAgents.openaiApiKeySet && (
              <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-cyan-400">Configured</span>
            )}
          </div>
        </div>

        <div>
          <label className="mb-1.5 block text-xs font-medium uppercase tracking-wider text-slate-400" htmlFor="gemini-key">
            Gemini API Key
          </label>
          <div className="relative">
            <input
              id="gemini-key"
              type="password"
              className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm text-white placeholder-slate-500 focus:border-cyan-500/50 focus:outline-none focus:ring-1 focus:ring-cyan-500/30"
              placeholder={aiAgents.geminiApiKeySet ? '••••••••••••••••••••••' : 'AIza...'}
              value={geminiKey}
              onChange={(e) => setGeminiKey(e.target.value)}
            />
            {aiAgents.geminiApiKeySet && (
              <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-cyan-400">Configured</span>
            )}
          </div>
        </div>

        <button
          type="button"
          className="flex w-full items-center justify-center gap-2 rounded-xl bg-cyan-500 px-4 py-2.5 text-sm font-semibold text-slate-950 transition hover:bg-cyan-400 disabled:opacity-50"
          disabled={isSaving || (!openaiKey && !geminiKey)}
          onClick={() => { void handleSave(); }}
        >
          {isSaving ? 'Saving...' : saved ? 'Saved!' : 'Save AI Config'}
        </button>
      </div>
    </div>
  );
}
