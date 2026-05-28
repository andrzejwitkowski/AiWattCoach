import { looksLikeMarkdown } from './looksLikeMarkdown';
import { MarkdownContent } from './MarkdownContent';

type SummaryTextContentProps = {
  text: string;
  className?: string;
};

export function SummaryTextContent({ text, className }: SummaryTextContentProps) {
  if (looksLikeMarkdown(text)) {
    return <MarkdownContent className={className}>{text}</MarkdownContent>;
  }

  return <div className={['whitespace-pre-wrap', className].filter(Boolean).join(' ')}>{text}</div>;
}
