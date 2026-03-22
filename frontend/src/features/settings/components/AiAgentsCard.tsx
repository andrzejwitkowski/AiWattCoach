import { useState } from 'react';
import { Bot, Eye, EyeOff, Save } from 'lucide-react';

import type { AiAgentsSettings, UpdateAiAgentsRequest } from '../types';

type AiAgentsCardProps = {
  settings: AiAgentsSettings;
  onSave: (data: UpdateAiAgentsRequest) => Promise<void>;
  isSaving: boolean;
};

export function AiAgentsCard({ settings, onSave, isSaving }: AiAgentsCardProps) {
  const [openaiKey, setOpenaiKey] = useState('');
  const [geminiKey, setGeminiKey] = useState('');
  const [showOpenai, setShowOpenai] = useState(false);
  const [showGemini, setShowGemini] = useState(false);

  const handleSave = async () => {
    await onSave({
      openaiApiKey: openaiKey || null,
      geminiApiKey: geminiKey || null,
    });
    setOpenaiKey('');
    setGeminiKey('');
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
              placeholder={settings.openaiApiKeySet ? 'Already configured' : 'sk-...'}
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
          {settings.openaiApiKeySet && (
            <p className="mt-1.5 text-xs text-emerald-400">Configured: {settings.openaiApiKey}</p>
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
              placeholder={settings.geminiApiKeySet ? 'Already configured' : 'AI...'}
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
          {settings.geminiApiKeySet && (
            <p className="mt-1.5 text-xs text-emerald-400">Configured: {settings.geminiApiKey}</p>
          )}
        </div>
      </div>

      <button
        className="mt-6 w-full flex items-center justify-center gap-2 bg-cyan-400 text-slate-950 font-semibold rounded-xl py-3 text-sm hover:bg-cyan-300 transition disabled:opacity-60 disabled:cursor-not-allowed"
        onClick={handleSave}
        disabled={isSaving}
        type="button"
      >
        {isSaving ? (
          'Saving...'
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
