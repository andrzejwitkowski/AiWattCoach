import { useEffect, useRef } from 'react';

import type { CoachChatProgressState, ConversationMessage } from '../types';
import { ChatMessage } from './ChatMessage';
import { ChatTypingIndicator } from './ChatTypingIndicator';

const AUTO_SCROLL_BOTTOM_THRESHOLD_PX = 80;

type ChatMessageListProps = {
  messages: ConversationMessage[];
  isCoachTyping: boolean;
  progressState?: CoachChatProgressState;
  onSendMessage?: (content: string) => Promise<boolean>;
};

export function ChatMessageList({
  messages,
  isCoachTyping,
  progressState = 'idle',
  onSendMessage,
}: ChatMessageListProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const endRef = useRef<HTMLDivElement | null>(null);
  const hasMountedRef = useRef(false);
  const shouldAutoScrollRef = useRef(true);
  const shouldShowProgressIndicator = progressState !== 'idle';

  function updateShouldAutoScroll() {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    shouldAutoScrollRef.current =
      container.scrollHeight - container.scrollTop - container.clientHeight < AUTO_SCROLL_BOTTOM_THRESHOLD_PX;
  }

  useEffect(() => {
    if (!hasMountedRef.current || shouldAutoScrollRef.current) {
      endRef.current?.scrollIntoView({ block: 'end' });
    }

    hasMountedRef.current = true;
  }, [isCoachTyping, messages, progressState]);

  return (
    <div
      ref={containerRef}
      className="no-scrollbar flex-1 space-y-4 overflow-y-auto px-6 py-6"
      data-testid="coach-chat-message-list"
      onScroll={updateShouldAutoScroll}
    >
      {messages.map((message) => (
        <ChatMessage key={message.id} message={message} onSendMessage={onSendMessage} />
      ))}
      {shouldShowProgressIndicator ? <ChatTypingIndicator progressState={progressState} /> : null}
      {!shouldShowProgressIndicator && isCoachTyping ? <ChatTypingIndicator /> : null}
      <div ref={endRef} />
    </div>
  );
}
