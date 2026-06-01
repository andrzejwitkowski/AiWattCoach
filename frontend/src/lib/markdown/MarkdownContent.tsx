import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { Components } from 'react-markdown';

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
  p: ({ children }) => <p className="mb-2 whitespace-pre-wrap break-words leading-7">{children}</p>,
  blockquote: ({ children }) => (
    <blockquote className="my-2 border-l-2 border-amber-400/30 pl-3 italic text-slate-300">
      {children}
    </blockquote>
  ),
};

type MarkdownContentProps = {
  children: string;
  className?: string;
};

export function MarkdownContent({ children, className }: MarkdownContentProps) {
  return (
    <div className={className}>
      <Markdown remarkPlugins={[remarkGfm]} components={mdComponents}>
        {children}
      </Markdown>
    </div>
  );
}
