import { useTranslation } from 'react-i18next';

export function ChatTypingIndicator() {
  const { t } = useTranslation();

  return (
    <div className="flex justify-start">
      <div className="rounded-2xl rounded-tl-none border border-white/10 bg-white/5 px-4 py-3 text-sm text-slate-300">
        <div className="flex items-center gap-3">
          <span>{t('coach.coachTyping')}</span>
          <span className="flex gap-1">
            <span className="h-2 w-2 animate-pulse rounded-full bg-cyan-300" />
            <span className="h-2 w-2 animate-pulse rounded-full bg-cyan-300 [animation-delay:150ms]" />
            <span className="h-2 w-2 animate-pulse rounded-full bg-cyan-300 [animation-delay:300ms]" />
          </span>
        </div>
      </div>
    </div>
  );
}
