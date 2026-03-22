import { useState } from 'react';
import { Bot, Eye, EyeOff, Save } from 'lucide-react';
import type { UserSettingsResponse } from '../types';
import { updateAiAgents } from '../api/settings';

type AiAgentsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

export function AiAgentsCard({ settings, apiBaseUrl, onSave }: AiAgentsCardProps) {
  const [openaiKey, setOpenaiKey] = useState('');
  const [geminiKey, setGeminiKey] = useState('');
  const [showOpenai, setShowOpenai] = useState(false);
  const [showGemini, setShowGemini] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const aiAgents = settings.aiAgents;

  const handleSave = async () => {
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
  };

  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-6 backdrop-blur">
      <div className="flex items-start gap-4">
        <div className="w-10 h-10 rounded-xl bg-slate-800 flex items-center justify-center shrink-0">
          <Bot size={20} className="text-cyan-400" />
        </div>
        <div className="flex-1">
          <div className="flex items-center gap-2 flex-wrap">
            <h2 className="text-xl font-bold text-white">AI Agents</h2>
            <span className="text-[10px] font-semibold bg-cyan-400/20 text-cyan-300 rounded-full px-2 py-0.5 uppercase tracking-wider">
              BYOK
            </span>
          </div>
          <p className="text-[10px] uppercase tracking-[0.2em] text-slate-500 mt-0.5">
            Performance Intelligence
          </p>
        </div>
      </div>

      <p className="mt-4 text-sm text-slate-300 leading-relaxed">
        Configure your own API keys for AI model access. Keys are stored securely and masked on display.
      </p>

      <div className="mt-6 space-y-4">
        <div>
          <label className="block text-xs uppercase tracking-widest text-slate-400 mb-2">
            OpenAI API Key
          </label>
          <div className="relative">
            <input
              className="w-full bg-slate-900/60 border border-white/10 rounded-xl px-4 py-3 pr-10 text-slate-200 text-sm placeholder:text-slate-600 focus:outline-none focus:border-cyan-400/50 transition"
              type={showOpenai ? 'text' : 'password'}
              placeholder={aiAgents.openaiApiKeySet ? 'Already configured' : 'sk-...'}
              value={openaiKey}
              onChange={(e) => setOpenaiKey(e.target.value)}
            />
            <button
              className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-200 transition"
              onClick={() => setShowOpenai((v) => !v)}
              type="button"
              aria-label={showOpenai ? 'Hide key' : 'Show key'}
            >
              {showOpenai ? <EyeOff size={16} /> : <Eye size={16} />}
            </button>
          </div>
          {aiAgents.openaiApiKeySet && (
            <p className="mt-1.5 text-xs text-emerald-400">API key is configured</p>
          )}
        </div>

        <div>
          <label className="block text-xs uppercase tracking-widest text-slate-400 mb-2">
            Gemini API Key
          </label>
          <div className="relative">
            <input
              className="w-full bg-slate-900/60 border border-white/10 rounded-xl px-4 py-3 pr-10 text-slate-200 text-sm placeholder:text-slate-600 focus:outline-none focus:border-cyan-400/50 transition"
              type={showGemini ? 'text' : 'password'}
              placeholder={aiAgents.geminiApiKeySet ? 'Already configured' : 'AIza...'}
              value={geminiKey}
              onChange={(e) => setGeminiKey(e.target.value)}
            />
            <button
              className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-200 transition"
              onClick={() => setShowGemini((v) => !v)}
              type="button"
              aria-label={showGemini ? 'Hide key' : 'Show key'}
            >
              {showGemini ? <EyeOff size={16} /> : <Eye size={16} />}
            </button>
          </div>
          {aiAgents.geminiApiKeySet && (
            <p className="mt-1.5 text-xs text-emerald-400">API key is configured</p>
          )}
        </div>
      </div>

      {saveError && (
        <div className="mt-4 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
          {saveError}
        </div>
      )}

      <button
        className="mt-6 w-full flex items-center justify-center gap-2 bg-cyan-400 text-slate-950 font-semibold rounded-xl py-3 text-sm hover:bg-cyan-300 transition disabled:opacity-60 disabled:cursor-not-allowed"
        onClick={() => { void handleSave(); }}
        disabled={isSaving || (!openaiKey.trim() && !geminiKey.trim())}
        type="button"
      >
        {isSaving ? (
          'Saving...'
        ) : saved ? (
          'Saved!'
        ) : (
          <>
            <Save size={15} />
            Save AI Config
          </>
        )}
      </button>
    </div>
  );
}
