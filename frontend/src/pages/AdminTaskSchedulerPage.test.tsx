import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ApiBaseUrlProvider } from '../lib/apiBaseUrl';
import { AdminTaskSchedulerPage } from './AdminTaskSchedulerPage';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

const originalFetch = global.fetch;

afterEach(() => {
  cleanup();
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('AdminTaskSchedulerPage', () => {
  it('renders tasks, details, refresh, pagination, sorting and retry', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(jsonResponse({
        items: [
          taskPayload('task-failed', 'failed', 300),
          taskPayload('task-done', 'completed', 200),
          taskPayload('task-timeout', 'timed_out', 100),
        ],
        nextOffset: 20,
        previousOffset: null,
        limit: 20,
      }))
      .mockResolvedValueOnce(jsonResponse(taskPayload('task-failed', 'failed', 300)))
      .mockResolvedValueOnce(jsonResponse(taskPayload('task-failed', 'queued', 300)))
      .mockResolvedValueOnce(jsonResponse({
        items: [taskPayload('task-refreshed', 'running', 400)],
        nextOffset: 20,
        previousOffset: null,
        limit: 20,
      }))
      .mockResolvedValueOnce(jsonResponse(taskPayload('task-failed', 'queued', 300)))
      .mockResolvedValueOnce(jsonResponse({
        items: [taskPayload('task-next', 'queued', 50)],
        nextOffset: null,
        previousOffset: 0,
        limit: 20,
      }))
      .mockResolvedValueOnce(jsonResponse({
        items: [taskPayload('task-sorted', 'queued', 500)],
        nextOffset: null,
        previousOffset: null,
        limit: 20,
      }));
    global.fetch = fetchMock as typeof fetch;

    renderAdminTaskSchedulerPage();

    const failedRow = await screen.findByText('task-failed');
    expect(failedRow.closest('tr')?.className).toContain('bg-rose');
    expect(screen.getByText('task-done').closest('tr')?.className).toContain('bg-emerald');
    expect(screen.getByText('task-timeout').closest('tr')?.className).toContain('bg-amber');
    expect(screen.getByRole('button', { name: /Lease expires/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Heartbeat/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Timeout/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Leader only/ })).toBeInTheDocument();

    await userEvent.click(failedRow);
    expect(await screen.findByText('Payload')).toBeInTheDocument();
    expect(screen.getByText('Execution timeout')).toBeInTheDocument();

    const failedTableRow = failedRow.closest('tr');
    expect(failedTableRow).not.toBeNull();
    await userEvent.click(within(failedTableRow as HTMLTableRowElement).getByRole('button', { name: 'adminTaskScheduler.retry' }));

    await waitFor(() => {
      expect(screen.getAllByText('queued').length).toBeGreaterThan(0);
    });

    await userEvent.click(screen.getByRole('button', { name: /adminTaskScheduler.refresh/i }));
    await waitFor(() => {
      expect(screen.getByText('task-refreshed')).toBeInTheDocument();
    });

    await userEvent.click(screen.getByRole('button', { name: 'adminTaskScheduler.next' }));
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(expect.stringContaining('offset=20'), expect.any(Object));
    });

    await userEvent.click(screen.getByRole('button', { name: /Status/ }));
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(expect.stringContaining('sortField=status'), expect.any(Object));
    });
  });

  it('shows localized load and retry errors instead of raw transport errors', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(new Response('unavailable', { status: 503 }))
      .mockResolvedValueOnce(jsonResponse({
        items: [taskPayload('task-failed', 'failed', 300)],
        nextOffset: null,
        previousOffset: null,
        limit: 20,
      }))
      .mockResolvedValueOnce(new Response('conflict', { status: 409 }));
    global.fetch = fetchMock as typeof fetch;

    renderAdminTaskSchedulerPage();

    expect(await screen.findByText('adminTaskScheduler.loadError')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /adminTaskScheduler.refresh/i }));
    const failedRow = await screen.findByText('task-failed');
    const failedTableRow = failedRow.closest('tr');
    expect(failedTableRow).not.toBeNull();
    await userEvent.click(within(failedTableRow as HTMLTableRowElement).getByRole('button', { name: 'adminTaskScheduler.retry' }));

    expect(await screen.findByText('adminTaskScheduler.retryError')).toBeInTheDocument();
    expect(screen.queryByText(/failed: 409/i)).not.toBeInTheDocument();
  });
});

function renderAdminTaskSchedulerPage() {
  render(
    <ApiBaseUrlProvider value="">
      <AdminTaskSchedulerPage />
    </ApiBaseUrlProvider>,
  );
}

function jsonResponse(payload: unknown) {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}

function taskPayload(id: string, status: string, createdAt: number) {
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
    nextAttemptAtEpochSeconds: createdAt,
    claimedBy: null,
    leaseExpiresAtEpochSeconds: null,
    lastHeartbeatAtEpochSeconds: null,
    executionTimeoutSeconds: 30,
    timedOutAtEpochSeconds: status === 'timed_out' ? createdAt + 10 : null,
    leaderOnly: false,
    createdAtEpochSeconds: createdAt,
    updatedAtEpochSeconds: createdAt,
    startedAtEpochSeconds: null,
    finishedAtEpochSeconds: status === 'queued' || status === 'running' ? null : createdAt + 10,
  };
}
