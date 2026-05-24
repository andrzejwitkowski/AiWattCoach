import { renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  loadAdminSchedulerTask,
  loadAdminSchedulerTasks,
  retryAdminSchedulerTask,
  useAdminTaskSchedulerApi,
} from './api';
import { ApiBaseUrlProvider } from '../../lib/apiBaseUrl';

const originalFetch = global.fetch;

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('admin task scheduler api', () => {
  it('loads a paged task list', async () => {
    const fetchMock = mockFetch({
      items: [taskPayload('task-1', 'completed')],
      nextOffset: 20,
      previousOffset: null,
      limit: 20,
    });

    const page = await loadAdminSchedulerTasks('', {
      limit: 20,
      offset: 0,
      sortField: 'createdAt',
      sortDirection: 'desc',
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/admin/task-scheduler/tasks?limit=20&offset=0&sortField=createdAt&sortDirection=desc', expect.objectContaining({
      method: 'GET',
      credentials: 'include',
    }));
    expect(page.items[0].id).toBe('task-1');
    expect(page.nextOffset).toBe(20);
  });

  it('loads task details', async () => {
    mockFetch(taskPayload('task-2', 'running'));

    const task = await loadAdminSchedulerTask('', 'task-2');

    expect(task.status).toBe('running');
  });

  it('parses retry response', async () => {
    mockFetch(taskPayload('task-3', 'queued'));

    const task = await retryAdminSchedulerTask('', 'task-3');

    expect(task.status).toBe('queued');
  });

  it('validates list params before sending the request', async () => {
    global.fetch = vi.fn() as typeof fetch;

    await expect(loadAdminSchedulerTasks('', {
      limit: 0,
      offset: 0,
      sortField: 'createdAt',
      sortDirection: 'desc',
    })).rejects.toThrow('Invalid task list limit');

    expect(global.fetch).not.toHaveBeenCalled();
  });

  it('wraps apiBaseUrl-bound calls in a feature hook', async () => {
    const fetchMock = mockFetch({
      items: [taskPayload('task-1', 'completed')],
      nextOffset: 20,
      previousOffset: null,
      limit: 20,
    });

    const { result } = renderHook(() => useAdminTaskSchedulerApi(), {
      wrapper: wrapper(''),
    });

    const page = await result.current.loadAdminSchedulerTasks({
      limit: 20,
      offset: 0,
      sortField: 'createdAt',
      sortDirection: 'desc',
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/admin/task-scheduler/tasks?limit=20&offset=0&sortField=createdAt&sortDirection=desc', expect.objectContaining({
      method: 'GET',
      credentials: 'include',
    }));
    expect(page.items[0].id).toBe('task-1');
  });
});

function wrapper(apiBaseUrl: string) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(ApiBaseUrlProvider, { value: apiBaseUrl }, children);
  };
}

function mockFetch(payload: unknown) {
  const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
    .mockResolvedValue(new Response(JSON.stringify(payload), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    }));
  global.fetch = fetchMock as typeof fetch;
  return fetchMock;
}

function taskPayload(id: string, status: string) {
  return {
    id,
    userId: 'user-1',
    taskType: 'summary',
    status,
    payload: { id },
    checkpoint: null,
    retryStrategy: { kind: 'fixed', maxAttempts: 3, delaySeconds: 30 },
    dedupeKey: `dedupe-${id}`,
    errorMessage: status === 'failed' ? 'failed' : null,
    attemptCount: 1,
    nextAttemptAtEpochSeconds: 100,
    claimedBy: null,
    leaseExpiresAtEpochSeconds: null,
    lastHeartbeatAtEpochSeconds: null,
    executionTimeoutSeconds: 30,
    timedOutAtEpochSeconds: null,
    leaderOnly: false,
    createdAtEpochSeconds: 100,
    updatedAtEpochSeconds: 100,
    startedAtEpochSeconds: null,
    finishedAtEpochSeconds: null,
  };
}
