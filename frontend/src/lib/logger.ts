type FrontendLogLevel = 'info' | 'warn' | 'error';

const TRACE_VERSION = '00';
const TRACE_FLAGS = '01';

let frontendTraceparent: string | null = null;
let consolePatched = false;

function randomHex(byteCount: number): string {
  const bytes = new Uint8Array(byteCount);

  if (globalThis.crypto?.getRandomValues) {
    globalThis.crypto.getRandomValues(bytes);
  } else {
    for (let index = 0; index < byteCount; index += 1) {
      bytes[index] = Math.floor(Math.random() * 256);
    }
  }

  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function formatLogPart(value: unknown): string {
  if (typeof value === 'string') {
    return value;
  }

  if (value instanceof Error) {
    return value.stack ?? value.message;
  }

  if (typeof value === 'object' && value !== null) {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }

  return String(value);
}

function formatLogMessage(parts: unknown[]): string {
  return parts.map((part) => formatLogPart(part)).join(' ');
}

export function getFrontendTraceparent(): string {
  if (!frontendTraceparent) {
    frontendTraceparent = `${TRACE_VERSION}-${randomHex(16)}-${randomHex(8)}-${TRACE_FLAGS}`;
  }

  return frontendTraceparent;
}

export async function sendFrontendLog(level: FrontendLogLevel, parts: unknown[]): Promise<void> {
  const message = formatLogMessage(parts);
  const payload = JSON.stringify({ level, message });
  const traceparent = getFrontendTraceparent();

  if (typeof navigator.sendBeacon === 'function') {
    const beaconBody = new Blob([payload], { type: 'application/json' });
    if (navigator.sendBeacon('/api/logs', beaconBody)) {
      return;
    }
  }

  if (typeof fetch !== 'function') {
    return;
  }

  await fetch('/api/logs', {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/json',
      traceparent,
    },
    body: payload,
    credentials: 'same-origin',
    keepalive: true,
  });
}

export function patchConsoleForwarding(): void {
  if (consolePatched) {
    return;
  }

  const originalInfo = console.info.bind(console);
  const originalWarn = console.warn.bind(console);
  const originalError = console.error.bind(console);

  console.info = (...parts: unknown[]) => {
    originalInfo(...parts);
    void sendFrontendLog('info', parts);
  };

  console.warn = (...parts: unknown[]) => {
    originalWarn(...parts);
    void sendFrontendLog('warn', parts);
  };

  console.error = (...parts: unknown[]) => {
    originalError(...parts);
    void sendFrontendLog('error', parts);
  };

  consolePatched = true;
}
