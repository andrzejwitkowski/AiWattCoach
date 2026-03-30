import { afterEach, vi } from 'vitest';

const originalFetch = global.fetch;

export function createFetchMock() {
  return vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>();
}

export function useFetchMock(fetchMock: ReturnType<typeof createFetchMock>) {
  global.fetch = fetchMock as typeof fetch;
  return fetchMock;
}

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});
