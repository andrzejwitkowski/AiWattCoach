export function parseKeyValueLines(text: string): Record<string, string> {
  const result: Record<string, string> = {};
  let currentKey = '';
  let currentValue = '';

  for (const line of text.split('\n')) {
    const eqIdx = line.indexOf('=');

    if (eqIdx >= 0 && currentKey === '') {
      currentKey = line.slice(0, eqIdx);
      currentValue = line.slice(eqIdx + 1);
    } else if (eqIdx >= 0 && currentKey !== '') {
      if (currentValue.startsWith('{') && !isBalancedBrace(currentValue)) {
        currentValue += '\n' + line;
      } else {
        result[currentKey] = currentValue;
        currentKey = line.slice(0, eqIdx);
        currentValue = line.slice(eqIdx + 1);
      }
    } else {
      currentValue += '\n' + line;
      if (currentValue.startsWith('{') && isBalancedBrace(currentValue)) {
        result[currentKey] = currentValue;
        currentKey = '';
        currentValue = '';
      }
    }
  }

  if (currentKey !== '') {
    result[currentKey] = currentValue;
  }

  return result;
}

function isBalancedBrace(s: string): boolean {
  let depth = 0;
  let inString = false;
  for (let i = 0; i < s.length; i++) {
    const ch = s[i];
    if (inString) {
      if (ch === '\\') { i++; continue; }
      if (ch === '"') inString = false;
      continue;
    }
    if (ch === '"') { inString = true; continue; }
    if (ch === '{') depth++;
    if (ch === '}') depth--;
  }
  return depth === 0;
}

export function tryParseJson<T = unknown>(text: string): T | null {
  try {
    return JSON.parse(text) as T;
  } catch {
    return null;
  }
}
