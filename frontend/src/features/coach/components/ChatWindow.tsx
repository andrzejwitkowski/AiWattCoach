import { useTranslation } from 'react-i18next';

import type { ConversationMessage } from '../types';
import { ChatHeader } from './ChatHeader';
import { ChatInput } from './ChatInput';
import { ChatMessageList } from './ChatMessageList';

type ChatWindowProps = {
  messages: ConversationMessage[];
  isCoachTyping: boolean;
  isConnected: boolean;
  hasSelectedWorkout: boolean;
  error: string | null;
  inputDisabled?: boolean;
  onSendMessage: (content: string) => Promise<void>;
};

export function ChatWindow({
  messages,
  isCoachTyping,
  isConnected,
  hasSelectedWorkout,
  error,
  inputDisabled = false,
  onSendMessage,
}: ChatWindowProps) {
  const { t } = useTranslation();

  return (
    <section className="glass-panel flex h-[38rem] flex-col rounded-2xl border border-white/10 bg-[#111417]/80">
      <ChatHeader isConnected={isConnected} hasSelectedWorkout={hasSelectedWorkout} />
      {error ? (
        <div className="mx-6 mt-6 rounded-2xl border border-red-400/25 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          {error}
        </div>
      ) : null}
      {hasSelectedWorkout ? (
        <>
          <ChatMessageList messages={messages} isCoachTyping={isCoachTyping} />
          <ChatInput disabled={inputDisabled} onSend={onSendMessage} />
        </>
      ) : (
        <div className="flex flex-1 items-center justify-center px-6 text-center text-slate-400">
          {t('coach.selectWorkoutPrompt')}
        </div>
      )}
    </section>
  );
}
