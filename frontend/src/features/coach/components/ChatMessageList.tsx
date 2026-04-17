import { useEffect, useRef } from 'react';

import type { CoachChatProgressState, ConversationMessage } from '../types';
import { ChatMessage } from './ChatMessage';
import { ChatTypingIndicator } from './ChatTypingIndicator';

type ChatMessageListProps = {
  messages: ConversationMessage[];
  isCoachTyping: boolean;
  progressState?: CoachChatProgressState;
};

export function ChatMessageList({ messages, isCoachTyping, progressState = 'idle' }: ChatMessageListProps) {
  const endRef = useRef<HTMLDivElement | null>(null);
  const shouldShowProgressIndicator = progressState !== 'idle';

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: 'end' });
  }, [isCoachTyping, messages, progressState]);

  return (
    <div className="no-scrollbar flex-1 space-y-4 overflow-y-auto px-6 py-6">
      {messages.map((message) => (
        <ChatMessage key={message.id} message={message} />
      ))}
      {shouldShowProgressIndicator ? <ChatTypingIndicator progressState={progressState} /> : null}
      {!shouldShowProgressIndicator && isCoachTyping ? <ChatTypingIndicator /> : null}
      <div ref={endRef} />
    </div>
  );
}
