import { Bot, Plus, Sparkles, X } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { ChatInput } from '../../coach/components/ChatInput';
import { ChatMessageList } from '../../coach/components/ChatMessageList';
import { useCalendarCoachChat } from '../hooks/useCalendarCoachChat';

type CalendarCoachModalProps = {
  isOpen: boolean;
  onClose: () => void;
};

export function CalendarCoachModal({ isOpen, onClose }: CalendarCoachModalProps) {
  const { t } = useTranslation();
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);
  const savedTriggerRef = useRef<HTMLElement | null>(null);
  const {
    messages,
    isLoading,
    isStartingNewConversation,
    isConnected,
    isCoachTyping,
    error,
    sendMessage,
    startNewConversation,
  } = useCalendarCoachChat({ isOpen });

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    savedTriggerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';

    closeButtonRef.current?.focus();

    const trapFocus = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
        return;
      }

      if (event.key !== 'Tab') {
        return;
      }

      const dialog = dialogRef.current;
      if (!dialog) {
        return;
      }

      const focusableElements = Array.from(
        dialog.querySelectorAll<HTMLElement>(
          'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      ).filter((element) => !element.hasAttribute('disabled'));

      const firstElement = focusableElements[0];
      const lastElement = focusableElements[focusableElements.length - 1];

      if (!firstElement || !lastElement) {
        event.preventDefault();
        dialog.focus();
        return;
      }

      const activeElement = document.activeElement;

      if (event.shiftKey) {
        if (activeElement === firstElement || !dialog.contains(activeElement)) {
          event.preventDefault();
          lastElement.focus();
        }
        return;
      }

      if (activeElement === lastElement || !dialog.contains(activeElement)) {
        event.preventDefault();
        firstElement.focus();
      }
    };

    window.addEventListener('keydown', trapFocus);

    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener('keydown', trapFocus);
      savedTriggerRef.current?.focus();
    };
  }, [isOpen, onClose]);

  if (!isOpen) {
    return null;
  }

  const statusLabel = isLoading
    ? t('calendar.coachModalStatusLoading')
    : isCoachTyping
      ? t('calendar.coachModalStatusTyping')
      : isConnected
        ? t('calendar.coachModalStatusConnected')
        : t('calendar.coachModalStatusReady');

  const actionDisabled = isLoading || isStartingNewConversation;
  const inputDisabled = isLoading || isStartingNewConversation;
  const hasMessages = messages.length > 0;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-[#05070a]/78 px-4 py-6 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="calendar-coach-title"
        ref={dialogRef}
        tabIndex={-1}
        className="flex max-h-[min(88vh,58rem)] w-full max-w-5xl flex-col overflow-hidden rounded-[1.75rem] border border-white/8 bg-[linear-gradient(180deg,rgba(28,32,36,0.98),rgba(15,18,20,0.98))] shadow-[0_40px_120px_rgba(0,0,0,0.58)]"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className="flex items-start justify-between gap-4 border-b border-white/6 px-6 py-5 md:px-8">
          <div className="flex min-w-0 items-start gap-4">
            <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-full border border-[#d2ff9a]/30 bg-[#263122] text-[#d2ff9a] shadow-[0_0_28px_rgba(210,255,154,0.14)]">
              <Bot size={24} />
            </div>
            <div className="min-w-0">
              <h2 id="calendar-coach-title" className="text-2xl font-black tracking-tight text-[#f9f9fd] md:text-[2rem]">
                {t('calendar.coachModalTitle')}
              </h2>
              <p className="mt-2 inline-flex items-center gap-2 text-[11px] font-black uppercase tracking-[0.28em] text-[#d2ff9a]">
                <span className={`h-2 w-2 rounded-full ${isConnected ? 'bg-[#d2ff9a]' : 'bg-slate-500'}`} />
                <span aria-live="polite">{statusLabel}</span>
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => {
                void startNewConversation();
              }}
              disabled={actionDisabled}
              className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-4 py-2 text-sm font-semibold text-slate-100 transition hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Plus size={16} />
              {t('calendar.coachNewConversation')}
            </button>
            <button
              ref={closeButtonRef}
              type="button"
              onClick={onClose}
              aria-label={t('calendar.closeCoach')}
              className="rounded-full border border-white/10 bg-white/5 p-2 text-slate-300 transition hover:bg-white/10 hover:text-white"
            >
              <X size={18} />
            </button>
          </div>
        </div>

        <div className="flex min-h-0 flex-1 flex-col bg-[#111417]">
          <div className="flex-1 overflow-y-auto px-6 py-6 md:px-8 md:py-8">
            <div className="mx-auto flex max-w-4xl flex-col gap-8">
              <div className="flex items-start gap-4">
                <div className="mt-1 flex h-10 w-10 shrink-0 items-center justify-center rounded-full border border-white/8 bg-white/5 text-[#d2ff9a]">
                  <Sparkles size={16} />
                </div>
                <div className="max-w-3xl rounded-[1.75rem] border border-white/6 bg-[#23272b] px-6 py-5 shadow-[0_18px_40px_rgba(0,0,0,0.18)]">
                  <p className="text-base leading-8 text-[#f3f4f6]">{t('calendar.coachModalIntro')}</p>
                  <div className="mt-5 grid gap-4 border-t border-white/6 pt-4 text-left sm:grid-cols-2">
                    <MetricPreview label={t('calendar.coachMetricFocus')} value={t('calendar.coachMetricFocusValue')} accent="text-[#00e3fd]" />
                    <MetricPreview label={t('calendar.coachMetricMode')} value={t('calendar.coachMetricModeValue')} accent="text-[#d2ff9a]" />
                  </div>
                </div>
              </div>

              {error ? (
                <div aria-live="polite" className="rounded-2xl border border-red-400/25 bg-red-500/10 px-4 py-3 text-sm text-red-200">
                  {error}
                </div>
              ) : null}

              {hasMessages ? (
                <div className="rounded-[1.6rem] border border-white/8 bg-[#14181b] shadow-[0_16px_35px_rgba(0,0,0,0.18)]">
                  <ChatMessageList messages={messages} isCoachTyping={isCoachTyping} />
                </div>
              ) : (
                <div className="flex justify-end">
                  <div className="max-w-3xl rounded-[1.6rem] border border-[#7ea855]/35 bg-[#1f261c] px-6 py-5 text-base leading-8 text-[#e4f4b7] shadow-[0_16px_35px_rgba(0,0,0,0.18)]">
                    {isLoading ? t('calendar.coachConversationLoading') : t('calendar.coachConversationEmptyState')}
                  </div>
                </div>
              )}
            </div>
          </div>

          <div className="border-t border-white/6 bg-[#15191c] px-6 py-5 md:px-8">
            <div className="mx-auto max-w-4xl">
              <ChatInput
                disabled={inputDisabled}
                ariaLabel={t('calendar.coachModalInputLabel')}
                placeholder={t('calendar.coachModalPlaceholder')}
                sendAriaLabel={t('calendar.coachSend')}
                onSend={sendMessage}
              />

              <p className="mt-3 text-sm text-slate-400">{t('calendar.coachModalNote')}</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function MetricPreview({
  label,
  value,
  accent,
}: {
  label: string;
  value: string;
  accent: string;
}) {
  return (
    <div>
      <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-slate-500">{label}</p>
      <p className={`mt-2 text-3xl font-black tracking-tight ${accent}`}>{value}</p>
    </div>
  );
}
