import { useState } from 'react';
import { SendHorizontal } from 'lucide-react';
import { useTranslation } from 'react-i18next';

type ChatInputProps = {
  disabled?: boolean;
  onSend: (content: string) => Promise<void>;
};

export function ChatInput({ disabled = false, onSend }: ChatInputProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState('');

  async function handleSend() {
    const trimmed = value.trim();

    if (!trimmed || disabled) {
      return;
    }

    setValue('');
    await onSend(trimmed);
  }

  return (
    <div className="px-6 pb-6 pt-2">
      <div className="relative">
        <textarea
          className="min-h-28 w-full resize-none rounded-2xl border border-white/10 bg-black/40 px-5 py-4 pr-16 text-base text-white outline-none transition placeholder:text-slate-500 focus:border-cyan-300/40"
          disabled={disabled}
          placeholder={t('coach.chatPlaceholder')}
          rows={3}
          value={value}
          onChange={(event) => {
            setValue(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.shiftKey) {
              event.preventDefault();
              void handleSend();
            }
          }}
        />
        <button
          type="button"
          aria-label={t('coach.sendMessage')}
          className="absolute bottom-4 right-4 flex h-11 w-11 items-center justify-center rounded-xl bg-cyan-300 text-slate-950 transition hover:bg-cyan-200 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={disabled || value.trim().length === 0}
          onClick={() => {
            void handleSend();
          }}
        >
          <SendHorizontal size={18} />
        </button>
      </div>
    </div>
  );
}
