import type { ConversationMessage } from '../types';

type ChatMessageProps = {
  message: ConversationMessage;
};

function formatTimestamp(epochSeconds: number): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(epochSeconds * 1000));
}

export function ChatMessage({ message }: ChatMessageProps) {
  const isUser = message.role === 'user';

  return (
    <div className={['flex', isUser ? 'justify-end' : 'justify-start'].join(' ')}>
      <div
        className={[
          'max-w-[85%] rounded-2xl border px-4 py-4',
          isUser
            ? 'rounded-tr-none border-cyan-300/20 bg-cyan-300/10 text-cyan-50'
            : 'rounded-tl-none border-white/10 bg-white/5 text-white',
        ].join(' ')}
      >
        <p className="whitespace-pre-wrap text-base leading-7">{message.content}</p>
        <p className="mt-3 text-[10px] font-medium uppercase tracking-[0.18em] text-slate-500">
          {formatTimestamp(message.createdAtEpochSeconds)}
        </p>
      </div>
    </div>
  );
}
