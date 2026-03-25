import { afterEach, describe, expect, it, vi } from 'vitest';

import { get } from './httpClient';
import { getJsonResponse } from './api/client';
import {
  generateTraceparent,
  patchConsoleForwarding,
  sendFrontendLog,
} from './logger';

const originalFetch = global.fetch;
const originalSendBeacon = navigator.sendBeacon;
const originalInfo = console.info;
const originalWarn = console.warn;
const originalError = console.error;
const originalBlob = globalThis.Blob;

afterEach(() => {
  global.fetch = originalFetch;
  navigator.sendBeacon = originalSendBeacon;
  console.info = originalInfo;
  console.warn = originalWarn;
  console.error = originalError;
  globalThis.Blob = originalBlob;
  vi.restoreAllMocks();
});

class InspectableBlob {
  public readonly type: string;

  constructor(
    private readonly parts: unknown[],
    options?: BlobPropertyBag,
  ) {
    this.type = options?.type ?? '';
  }

  async text(): Promise<string> {
    return this.parts.join('');
  }
}

function installInspectableBlob(): void {
  globalThis.Blob = InspectableBlob as unknown as typeof Blob;
}

async function readBlobText(blob: Blob): Promise<string> {
  return await (blob as unknown as InspectableBlob).text();
}

describe('logger', () => {
  it('generates a fresh traceparent per call with stable trace-id', () => {
    const first = generateTraceparent();
    const second = generateTraceparent();

    // Different parent-id per call
    expect(second).not.toBe(first);

    // Same trace-id across calls
    const firstParts = first.split('-');
    const secondParts = second.split('-');
    expect(firstParts[1]).toBe(secondParts[1]);

    // Valid W3C format
    expect(firstParts).toHaveLength(4);
    expect(firstParts[0]).toMatch(/^[0-9a-f]{2}$/);
    expect(firstParts[1]).toMatch(/^[0-9a-f]{32}$/);
    expect(firstParts[2]).toMatch(/^[0-9a-f]{16}$/);
    expect(firstParts[3]).toBe('01');

    // Different span-id
    expect(firstParts[2]).not.toBe(secondParts[2]);
  });

  it('sends frontend logs with sendBeacon when available', async () => {
    installInspectableBlob();
    const sendBeacon = vi.fn<(url: string | URL, data?: BodyInit | null) => boolean>().mockReturnValue(true);
    navigator.sendBeacon = sendBeacon;

    await sendFrontendLog('warn', ['rate limited', { attempt: 2 }]);

    expect(sendBeacon).toHaveBeenCalledTimes(1);
    expect(sendBeacon.mock.calls[0]?.[0]).toBe('/api/logs');

    const payload = sendBeacon.mock.calls[0]?.[1];
    expect(payload).toBeInstanceOf(Blob);
    expect((payload as Blob).type).toBe('application/json');
    const payloadText = await readBlobText(payload as Blob);
    expect(payloadText).toContain('"level":"warn"');
    expect(payloadText).toContain('rate limited');
    expect(payloadText).toContain('attempt');
    expect(payloadText).toContain(':2');
    expect(payloadText).toMatch(/"traceparent":"[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}"/);
  });

  it('falls back to fetch when sendBeacon is unavailable', async () => {
    navigator.sendBeacon = undefined as unknown as typeof navigator.sendBeacon;
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(new Response(null, { status: 202 }));

    global.fetch = fetchMock as typeof fetch;

    await sendFrontendLog('error', ['frontend exploded']);

    expect(fetchMock).toHaveBeenCalledWith('/api/logs', expect.objectContaining({
      method: 'POST',
      credentials: 'same-origin',
      keepalive: true,
    }));

    const callArgs = fetchMock.mock.calls[0]?.[1] as RequestInit;
    const sentBody = JSON.parse(callArgs.body as string);
    expect(sentBody.level).toBe('error');
    expect(sentBody.message).toBe('frontend exploded');
    expect(sentBody.traceparent).toMatch(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/);

    const headers = callArgs.headers as Record<string, string>;
    expect(headers.traceparent).toMatch(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/);
  });

  it('patches console methods and forwards messages once', async () => {
    installInspectableBlob();
    const infoSpy = vi.fn();
    console.info = infoSpy;

    const sendBeacon = vi.fn<(url: string | URL, data?: BodyInit | null) => boolean>().mockReturnValue(true);
    navigator.sendBeacon = sendBeacon;

    patchConsoleForwarding();
    patchConsoleForwarding();

    console.info('hello', { source: 'ui' });

    expect(infoSpy).toHaveBeenCalledWith('hello', { source: 'ui' });
    expect(sendBeacon).toHaveBeenCalledTimes(1);

    const payload = sendBeacon.mock.calls[0]?.[1] as Blob;
    const payloadText = await readBlobText(payload);
    expect(payloadText).toContain('"level":"info"');
    expect(payloadText).toContain('hello {\\"source\\":\\"ui\\"}');
    expect(payloadText).toMatch(/"traceparent":"[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}"/);
  });

  it('injects traceparent into both frontend fetch wrappers', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ ok: true }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ ok: true }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );

    global.fetch = fetchMock as typeof fetch;

    await get<{ ok: boolean }>('', '/api/test');
    await getJsonResponse<{ ok: boolean }>('/health');

    expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/test', {
      method: 'GET',
      headers: {
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: undefined,
    });

    expect(fetchMock).toHaveBeenNthCalledWith(2, '/health', {
      method: 'GET',
      headers: {
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
    });
  });
});
