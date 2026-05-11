import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { Components } from 'react-markdown';
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

const mdComponents: Components = {
  table: ({ children }) => (
    <div className="my-3 overflow-x-auto">
      <table className="min-w-full border-collapse border border-white/10 text-sm">
        {children}
      </table>
    </div>
  ),
  thead: ({ children }) => (
    <thead className="border-b border-white/10 bg-white/5">{children}</thead>
  ),
  th: ({ children }) => (
    <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wider text-slate-300">
      {children}
    </th>
  ),
  td: ({ children }) => (
    <td className="border-t border-white/5 px-3 py-2 text-slate-200">{children}</td>
  ),
  h1: ({ children }) => (
    <h3 className="mb-2 mt-4 text-base font-bold text-white">{children}</h3>
  ),
  h2: ({ children }) => (
    <h3 className="mb-1.5 mt-3 text-sm font-bold text-white/90">{children}</h3>
  ),
  h3: ({ children }) => (
    <h4 className="mb-1 mt-2 text-sm font-semibold text-white/80">{children}</h4>
  ),
  strong: ({ children }) => (
    <strong className="font-bold text-white">{children}</strong>
  ),
  ul: ({ children }) => <ul className="my-2 list-disc space-y-1 pl-5">{children}</ul>,
  ol: ({ children }) => <ol className="my-2 list-decimal space-y-1 pl-5">{children}</ol>,
  li: ({ children }) => <li className="text-slate-200">{children}</li>,
  hr: () => <hr className="my-4 border-white/10" />,
  code: ({ children }) => (
    <code className="rounded bg-white/10 px-1.5 py-0.5 text-xs text-amber-200">{children}</code>
  ),
  p: ({ children }) => <p className="mb-2 leading-7">{children}</p>,
  blockquote: ({ children }) => (
    <blockquote className="my-2 border-l-2 border-amber-400/30 pl-3 italic text-slate-300">
      {children}
    </blockquote>
  ),
};

export function ChatMessage({ message }: ChatMessageProps) {
  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';
  const isTool = message.role === 'tool';
  const isCoach = message.role === 'coach';
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
            <Markdown remarkPlugins={[remarkGfm]} components={mdComponents}>
              {message.content}
            </Markdown>
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
