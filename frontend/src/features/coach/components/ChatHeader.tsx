import { Bot } from 'lucide-react';
import { useTranslation } from 'react-i18next';

type ChatHeaderProps = {
  isConnected: boolean;
  hasSelectedWorkout: boolean;
  isSaved?: boolean;
  requiresRpe?: boolean;
  requiresAvailability?: boolean;
};

export function ChatHeader({
  isConnected,
  hasSelectedWorkout,
  isSaved = false,
  requiresRpe = false,
  requiresAvailability = false,
}: ChatHeaderProps) {
  const { t } = useTranslation();
  const statusLabel = !hasSelectedWorkout
    ? t('coach.selectWorkoutPrompt')
    : requiresAvailability
      ? t('coach.chatRequiresAvailability')
    : requiresRpe
      ? t('coach.chatRequiresRpe')
      : isSaved
        ? t('coach.chatSavedLocked')
    : isConnected
      ? t('coach.chatConnected')
      : t('coach.chatReady');

  return (
    <div className="flex items-center gap-4 border-b border-white/10 px-6 py-5">
      <div className="relative">
        <div className="flex h-12 w-12 items-center justify-center rounded-full border border-cyan-300/30 bg-cyan-300/10 text-cyan-200">
          <Bot size={20} />
        </div>
        <span
          className={[
            'absolute -bottom-0.5 -right-0.5 h-3.5 w-3.5 rounded-full border-2 border-[#111417]',
            isConnected ? 'bg-emerald-400' : 'bg-slate-600',
          ].join(' ')}
        />
      </div>
      <div>
        <p className="text-xl font-semibold text-white">{t('coach.chatTitle')}</p>
        <p className="text-sm text-cyan-200">{statusLabel}</p>
      </div>
    </div>
  );
}
