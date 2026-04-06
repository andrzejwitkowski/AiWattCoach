import { useEffect, useMemo, useState } from 'react';
import { AlertCircle, CheckCircle2, RefreshCw, ScrollText } from 'lucide-react';

import { generateAthleteSummary, loadAthleteSummary } from '../api/athleteSummary';
import type { AthleteSummaryResponse, UserSettingsResponse } from '../types';

type AthleteSummaryCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
};

export function AthleteSummaryCard({ settings, apiBaseUrl }: AthleteSummaryCardProps) {
  const [summary, setSummary] = useState<AthleteSummaryResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isGenerating, setIsGenerating] = useState(false);
  const [status, setStatus] = useState<{
    tone: 'neutral' | 'success' | 'error';
    label: string;
    message: string;
  } | null>(null);

  const canGenerate = useMemo(() => {
    const ai = settings.aiAgents;
    const hasProvider = Boolean(ai.selectedProvider && ai.selectedModel);
    const hasKey =
      ai.selectedProvider === 'openai'
        ? ai.openaiApiKeySet
        : ai.selectedProvider === 'gemini'
          ? ai.geminiApiKeySet
          : ai.selectedProvider === 'openrouter'
            ? ai.openrouterApiKeySet
            : false;
    const intervalsReady = settings.intervals.apiKeySet && Boolean(settings.intervals.athleteId);
    return hasProvider && hasKey && intervalsReady;
  }, [settings]);

  async function refreshSummary() {
    const loaded = await loadAthleteSummary(apiBaseUrl);
    setSummary(loaded);
    return loaded;
  }

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setIsLoading(true);
      try {
        await refreshSummary();
        if (!cancelled) {
          setStatus(null);
        }
      } catch (error) {
        if (!cancelled) {
          setStatus({
            tone: 'error',
            label: 'Load failed',
            message: error instanceof Error ? error.message : 'Failed to load athlete summary',
          });
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    }

    void load();

    return () => {
      cancelled = true;
    };
  }, [apiBaseUrl]);

  const handleGenerate = async () => {
    if (!canGenerate || isGenerating) return;
    setIsGenerating(true);
    setStatus({
      tone: 'neutral',
      label: 'Generating',
      message: 'Generating athlete summary...',
    });
    try {
      await generateAthleteSummary(apiBaseUrl);
      await refreshSummary();
      setStatus({
        tone: 'success',
        label: 'Updated',
        message: 'Athlete summary updated.',
      });
    } catch (error) {
      try {
        const latest = await refreshSummary();
        if (latest.exists && latest.summaryText) {
          setStatus({
            tone: 'success',
            label: 'Updated',
            message: 'Athlete summary updated.',
          });
          return;
        }
      } catch {
        // Fall through to the original generate error so the user sees the failed action.
      }

      setStatus({
        tone: 'error',
        label: 'Failed',
        message: error instanceof Error ? error.message : 'Failed to generate athlete summary',
      });
    } finally {
      setIsGenerating(false);
    }
  };

  const statusClasses =
    status?.tone === 'success'
      ? 'border-emerald-400/30 bg-emerald-500/10 text-emerald-200'
      : status?.tone === 'error'
        ? 'border-red-500/30 bg-red-500/10 text-red-200'
        : 'border-cyan-400/20 bg-cyan-400/10 text-cyan-100';

  const StatusIcon =
    status?.tone === 'success' ? CheckCircle2 : status?.tone === 'error' ? AlertCircle : RefreshCw;

  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-6 backdrop-blur">
      <div className="flex items-start gap-4">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-slate-800">
          <ScrollText size={20} className="text-cyan-400" />
        </div>
        <div className="flex-1">
          <h2 className="text-xl font-bold text-white">Athlete Summary</h2>
          <p className="mt-0.5 text-[10px] uppercase tracking-[0.2em] text-slate-500">
            Bird&apos;s-Eye Context
          </p>
        </div>
      </div>

      <p className="mt-4 text-sm leading-relaxed text-slate-300">
        Generate a cached, up-to-date textual summary of the athlete profile, training patterns,
        strengths, weaknesses, and coaching context.
      </p>

      <div className="mt-4 rounded-xl border border-white/10 bg-slate-900/50 p-3">
        <textarea
          aria-label="Athlete summary"
          className="min-h-48 w-full resize-none bg-transparent text-sm leading-relaxed text-slate-200 outline-none"
          value={summary?.summaryText ?? ''}
          placeholder={isLoading ? 'Loading athlete summary...' : 'No athlete summary generated yet.'}
          readOnly
        />
      </div>

      {summary?.generatedAtEpochSeconds ? (
        <p className="mt-2 text-xs text-slate-500">
          Last generated: {new Date(summary.generatedAtEpochSeconds * 1000).toLocaleString()}
          {summary.stale ? ' · stale' : ''}
        </p>
      ) : null}

      {!canGenerate ? (
        <div className="mt-4 rounded-xl border border-amber-400/20 bg-amber-400/10 px-4 py-3 text-sm text-amber-100">
          Configure both an active AI provider and Intervals.icu credentials to generate the athlete summary.
        </div>
      ) : null}

      {status ? (
        <div
          className={`mt-4 rounded-xl border px-4 py-3 text-sm ${statusClasses}`}
          role={status.tone === 'error' ? 'alert' : 'status'}
          aria-live={status.tone === 'error' ? 'assertive' : 'polite'}
        >
          <div className="flex items-start gap-3">
            <StatusIcon
              size={16}
              className={status.tone === 'neutral' ? 'mt-0.5 shrink-0 animate-spin' : 'mt-0.5 shrink-0'}
            />
            <div>
              <p className="text-[11px] font-semibold uppercase tracking-wider">{status.label}</p>
              <p className="mt-1">{status.message}</p>
            </div>
          </div>
        </div>
      ) : null}

      <div className="mt-6 flex gap-3">
        <button
          type="button"
          className="flex flex-1 items-center justify-center gap-2 rounded-xl bg-cyan-400 py-3 text-sm font-semibold text-slate-950 transition hover:bg-cyan-300 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={!canGenerate || isGenerating || isLoading}
          onClick={() => {
            void handleGenerate();
          }}
        >
          <RefreshCw size={15} className={isGenerating ? 'animate-spin' : undefined} />
          {isGenerating
            ? 'Generating...'
            : summary?.exists
              ? 'Refresh athlete summary'
              : 'Create athlete summary'}
        </button>
      </div>
    </div>
  );
}
