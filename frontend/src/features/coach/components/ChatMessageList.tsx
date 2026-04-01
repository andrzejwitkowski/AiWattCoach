import { useEffect, useRef } from 'react';

import type { ConversationMessage } from '../types';
import { ChatMessage } from './ChatMessage';
import { ChatTypingIndicator } from './ChatTypingIndicator';

type ChatMessageListProps = {
  messages: ConversationMessage[];
  isCoachTyping: boolean;
};

export function ChatMessageList({ messages, isCoachTyping }: ChatMessageListProps) {
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: 'end' });
  }, [isCoachTyping, messages]);

  return (
    <div className="no-scrollbar flex-1 space-y-4 overflow-y-auto px-6 py-6">
      {messages.map((message) => (
        <ChatMessage key={message.id} message={message} />
      ))}
      {isCoachTyping ? <ChatTypingIndicator /> : null}
      <div ref={endRef} />
    </div>
  );
}
