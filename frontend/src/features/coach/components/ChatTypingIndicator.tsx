import { useTranslation } from 'react-i18next';

import type { CoachChatProgressState } from '../types';

type ChatTypingIndicatorProps = {
  progressState?: CoachChatProgressState;
};

export function ChatTypingIndicator({ progressState = 'idle' }: ChatTypingIndicatorProps) {
  const { t } = useTranslation();
  const label = progressState === 'saving-summary'
    ? t('coach.chatSavingSummary')
    : progressState === 'awaiting-reply'
      ? t('coach.chatAwaitingReply')
      : t('coach.coachTyping');
  const helperText = progressState === 'saving-summary'
    ? t('coach.chatSavingSummaryHint')
    : null;

  return (
    <div className="flex justify-start">
      <div className="rounded-2xl rounded-tl-none border border-white/10 bg-white/5 px-4 py-3 text-sm text-slate-300">
        <div className="flex items-center gap-3">
          <span>{label}</span>
          <span className="flex gap-1">
            <span className="h-2 w-2 animate-pulse rounded-full bg-cyan-300 motion-reduce:animate-none" />
            <span className="h-2 w-2 animate-pulse rounded-full bg-cyan-300 motion-reduce:animate-none [animation-delay:150ms]" />
            <span className="h-2 w-2 animate-pulse rounded-full bg-cyan-300 motion-reduce:animate-none [animation-delay:300ms]" />
          </span>
        </div>
        {helperText ? <p className="mt-2 max-w-md text-xs leading-5 text-slate-400">{helperText}</p> : null}
      </div>
    </div>
  );
}
