import { vi } from 'vitest';

export function mockFetch(payload: unknown) {
  const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
    .mockResolvedValue(
      new Response(JSON.stringify(payload), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    );
  global.fetch = fetchMock as typeof fetch;
  return fetchMock;
}
