import { MarkdownContent } from '../../../lib/markdown/MarkdownContent';
import { CoachQuestionnaire } from './CoachQuestionnaire';
import type { ConversationMessage } from '../types';

type ChatMessageProps = {
  message: ConversationMessage;
  onSendMessage?: (content: string) => Promise<boolean>;
};

function formatTimestamp(epochSeconds: number): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(epochSeconds * 1000));
}

export function ChatMessage({ message, onSendMessage }: ChatMessageProps) {
  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';
  const isTool = message.role === 'tool';
  const isCoach = message.role === 'coach';
  const questions = message.questions ?? [];
  const toolCallDisplay = message.toolCall?.argumentsPreview ?? message.toolCall?.argumentsJson;
  const containerClassName = ['flex', isUser ? 'justify-end' : 'justify-start'].join(' ');
  const bubbleClassName = [
    'max-w-[85%] rounded-2xl border px-4 py-4',
    isUser
      ? 'rounded-tr-none border-cyan-300/20 bg-cyan-300/10 text-cyan-50'
      : isSystem
        ? 'border-amber-200/20 bg-amber-100/10 text-amber-50'
        : isTool
          ? 'border-indigo-300/20 bg-indigo-300/10 text-indigo-50'
        : 'rounded-tl-none border-white/10 bg-white/5 text-white',
  ].join(' ');

  return (
    <div className={containerClassName}>
      <div className={bubbleClassName} data-message-role={message.role}>
        {isTool && message.toolCall ? (
          <div className="space-y-3">
            <p className="text-xs font-semibold uppercase tracking-[0.18em] text-indigo-200/80">Tool</p>
            <p className="text-sm font-semibold text-indigo-50">{message.toolCall.name}</p>
            <pre className="overflow-x-auto whitespace-pre-wrap rounded-xl border border-indigo-200/10 bg-slate-950/40 p-3 text-xs leading-6 text-indigo-100/80">
              {toolCallDisplay}
            </pre>
          </div>
        ) : isCoach ? (
          <div className="text-base">
            <MarkdownContent>{message.content}</MarkdownContent>
            {questions.length > 0 && onSendMessage ? (
              <CoachQuestionnaire questions={questions} onSubmit={onSendMessage} />
            ) : null}
          </div>
        ) : (
          <p className="whitespace-pre-wrap text-base leading-7">{message.content}</p>
        )}
        <p className="mt-3 text-[10px] font-medium uppercase tracking-[0.18em] text-slate-500">
          {formatTimestamp(message.createdAtEpochSeconds)}
        </p>
      </div>
    </div>
  );
}
