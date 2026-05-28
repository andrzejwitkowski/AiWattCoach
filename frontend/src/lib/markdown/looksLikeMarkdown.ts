const MARKDOWN_SIGNAL_PATTERNS: RegExp[] = [
  /^#{1,6}\s/m,
  /\*\*[^*\n]+\*\*/,
  /(?<!\*)\*[^*\n]+\*(?!\*)/,
  /`[^`\n]+`/,
  /^\s*[-*+]\s+/m,
  /^\s*\d+\.\s+/m,
  /^\s*>\s+/m,
  /^(\*{3,}|-{3,}|_{3,})\s*$/m,
  /^\|.+\|.+\|/m,
];

export function looksLikeMarkdown(text: string): boolean {
  const trimmed = text.trim();
  if (!trimmed) {
    return false;
  }

  return MARKDOWN_SIGNAL_PATTERNS.some((pattern) => pattern.test(trimmed));
}
