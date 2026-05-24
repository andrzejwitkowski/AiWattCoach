import { useEffect, useState } from 'react';
import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { useApiBaseUrl } from '../lib/apiBaseUrl';
import {
  loadAdminSchedulerTask,
  loadAdminSchedulerTasks,
  retryAdminSchedulerTask,
} from '../features/admin-task-scheduler/api';
import type {
  ScheduledTask,
  SortDirection,
  TaskSortField,
} from '../features/admin-task-scheduler/types';

const PAGE_SIZE = 20;

const columns: Array<{ field: TaskSortField; label: string }> = [
  { field: 'id', label: 'ID' },
  { field: 'createdAt', label: 'Created' },
  { field: 'status', label: 'Status' },
  { field: 'taskType', label: 'Type' },
  { field: 'userId', label: 'User' },
  { field: 'attemptCount', label: 'Attempts' },
  { field: 'nextAttemptAt', label: 'Next attempt' },
  { field: 'startedAt', label: 'Started' },
  { field: 'finishedAt', label: 'Finished' },
  { field: 'updatedAt', label: 'Updated' },
  { field: 'claimedBy', label: 'Worker' },
  { field: 'leaseExpiresAt', label: 'Lease expires' },
  { field: 'lastHeartbeatAt', label: 'Heartbeat' },
  { field: 'executionTimeout', label: 'Timeout' },
  { field: 'timedOutAt', label: 'Timed out' },
  { field: 'leaderOnly', label: 'Leader only' },
  { field: 'errorMessage', label: 'Error' },
  { field: 'dedupeKey', label: 'Dedupe' },
];

export function AdminTaskSchedulerPage() {
  const apiBaseUrl = useApiBaseUrl();
  const { t } = useTranslation();
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [selectedTask, setSelectedTask] = useState<ScheduledTask | null>(null);
  const [sortField, setSortField] = useState<TaskSortField>('createdAt');
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc');
  const [offset, setOffset] = useState(0);
  const [nextOffset, setNextOffset] = useState<number | null>(null);
  const [previousOffset, setPreviousOffset] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [retryingTaskId, setRetryingTaskId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    void loadTaskPage(apiBaseUrl, offset, sortField, sortDirection)
      .then((page) => {
        if (cancelled) return;
        setTasks(page.items);
        setNextOffset(page.nextOffset);
        setPreviousOffset(page.previousOffset);
        setError(null);
      })
      .catch(() => {
        if (cancelled) return;
        setError(t('adminTaskScheduler.loadError'));
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [apiBaseUrl, offset, sortField, sortDirection]);

  const refresh = async () => {
    setIsRefreshing(true);
    try {
      const [page, detail] = await Promise.all([
        loadTaskPage(apiBaseUrl, offset, sortField, sortDirection),
        selectedTaskId ? loadAdminSchedulerTask(apiBaseUrl, selectedTaskId).catch(() => null) : Promise.resolve(null),
      ]);
      setTasks(page.items);
      setNextOffset(page.nextOffset);
      setPreviousOffset(page.previousOffset);
      setSelectedTask(detail);
      if (selectedTaskId && !detail) {
        setSelectedTaskId(null);
      }
      setError(null);
    } catch {
      setError(t('adminTaskScheduler.loadError'));
    } finally {
      setIsRefreshing(false);
    }
  };

  const selectTask = async (task: ScheduledTask) => {
    setSelectedTaskId(task.id);
    setSelectedTask(task);
    try {
      setSelectedTask(await loadAdminSchedulerTask(apiBaseUrl, task.id));
    } catch {
      setSelectedTask(task);
    }
  };

  const retryTask = async (taskId: string) => {
    setRetryingTaskId(taskId);
    try {
      const task = await retryAdminSchedulerTask(apiBaseUrl, taskId);
      setTasks((current) => current.map((item) => item.id === task.id ? task : item));
      if (selectedTaskId === task.id) {
        setSelectedTask(task);
      }
      setError(null);
    } catch {
      setError(t('adminTaskScheduler.retryError'));
    } finally {
      setRetryingTaskId(null);
    }
  };

  const changeSort = (field: TaskSortField) => {
    setOffset(0);
    if (sortField === field) {
      setSortDirection((current) => current === 'asc' ? 'desc' : 'asc');
      return;
    }
    setSortField(field);
    setSortDirection(field.endsWith('At') ? 'desc' : 'asc');
  };

  return (
    <section className="space-y-6">
      <div className="flex flex-col gap-4 rounded-3xl border border-cyan-400/20 bg-slate-950/80 p-6 shadow-2xl shadow-cyan-950/30 md:flex-row md:items-center md:justify-between">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.28em] text-cyan-300">
            {t('adminTaskScheduler.kicker')}
          </p>
          <h2 className="mt-2 text-2xl font-bold text-white">{t('adminTaskScheduler.title')}</h2>
          <p className="mt-2 max-w-2xl text-sm text-slate-400">
            {t('adminTaskScheduler.description')}
          </p>
        </div>
        <button
          className="inline-flex items-center justify-center gap-2 rounded-xl border border-cyan-400/40 px-4 py-2 text-sm font-semibold text-cyan-100 transition hover:bg-cyan-400/10 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={isRefreshing}
          type="button"
          onClick={() => { void refresh(); }}
        >
          <RefreshCw size={16} className={isRefreshing ? 'animate-spin' : undefined} />
          {t('adminTaskScheduler.refresh')}
        </button>
      </div>

      {error && (
        <div className="rounded-2xl border border-rose-400/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-100">
          {error}
        </div>
      )}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.45fr)_minmax(22rem,0.75fr)]">
        <div className="rounded-3xl border border-white/10 bg-slate-950/70 p-4">
          <div className="overflow-x-auto">
            <table className="min-w-[1120px] w-full border-separate border-spacing-y-2 text-left text-sm">
              <thead className="text-xs uppercase tracking-wide text-slate-500">
                <tr>
                  {columns.map((column) => (
                    <th key={column.field} className="px-3 py-2">
                      <button
                        className="font-semibold transition hover:text-cyan-300"
                        type="button"
                        onClick={() => changeSort(column.field)}
                      >
                        {column.label}{sortField === column.field ? ` ${sortDirection === 'asc' ? '↑' : '↓'}` : ''}
                      </button>
                    </th>
                  ))}
                  <th className="px-3 py-2">{t('adminTaskScheduler.actions')}</th>
                </tr>
              </thead>
              <tbody>
                {tasks.map((task) => (
                  <tr
                    key={task.id}
                    aria-label={`Task ${task.id}`}
                    className={`${rowClassName(task)} cursor-pointer rounded-2xl transition hover:brightness-125`}
                    role="button"
                    tabIndex={0}
                    onClick={() => { void selectTask(task); }}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        void selectTask(task);
                      }
                    }}
                  >
                    <td className="rounded-l-2xl max-w-56 truncate px-3 py-3 font-medium text-slate-100">{task.id}</td>
                    <td className="px-3 py-3 font-medium text-slate-100">{formatEpoch(task.createdAtEpochSeconds)}</td>
                    <td className="px-3 py-3"><StatusBadge status={task.status} /></td>
                    <td className="px-3 py-3 text-slate-200">{task.taskType}</td>
                    <td className="px-3 py-3 text-slate-300">{task.userId}</td>
                    <td className="px-3 py-3 text-slate-300">{task.attemptCount}</td>
                    <td className="px-3 py-3 text-slate-300">{formatEpoch(task.nextAttemptAtEpochSeconds)}</td>
                    <td className="px-3 py-3 text-slate-300">{formatOptionalEpoch(task.startedAtEpochSeconds)}</td>
                    <td className="px-3 py-3 text-slate-300">{formatOptionalEpoch(task.finishedAtEpochSeconds)}</td>
                    <td className="px-3 py-3 text-slate-300">{formatEpoch(task.updatedAtEpochSeconds)}</td>
                    <td className="px-3 py-3 text-slate-300">{task.claimedBy ?? '-'}</td>
                    <td className="px-3 py-3 text-slate-300">{formatOptionalEpoch(task.leaseExpiresAtEpochSeconds)}</td>
                    <td className="px-3 py-3 text-slate-300">{formatOptionalEpoch(task.lastHeartbeatAtEpochSeconds)}</td>
                    <td className="px-3 py-3 text-slate-300">{task.executionTimeoutSeconds}s</td>
                    <td className="px-3 py-3 text-slate-300">{formatOptionalEpoch(task.timedOutAtEpochSeconds)}</td>
                    <td className="px-3 py-3 text-slate-300">{task.leaderOnly ? 'yes' : 'no'}</td>
                    <td className="max-w-64 truncate px-3 py-3 text-slate-400">{task.errorMessage ?? '-'}</td>
                    <td className="max-w-56 truncate px-3 py-3 text-slate-400">{task.dedupeKey}</td>
                    <td className="rounded-r-2xl px-3 py-3">
                      {canRetry(task) && (
                        <button
                          className="rounded-lg border border-white/20 px-3 py-1 text-xs font-semibold text-white transition hover:bg-white/10 disabled:opacity-50"
                          disabled={retryingTaskId === task.id}
                          type="button"
                          onClick={(event) => {
                            event.stopPropagation();
                            void retryTask(task.id);
                          }}
                        >
                          {retryingTaskId === task.id ? t('adminTaskScheduler.retrying') : t('adminTaskScheduler.retry')}
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {!isLoading && tasks.length === 0 && (
            <p className="py-10 text-center text-sm text-slate-400">{t('adminTaskScheduler.empty')}</p>
          )}
          {isLoading && (
            <p className="py-10 text-center text-sm text-slate-400">{t('adminTaskScheduler.loading')}</p>
          )}
          <div className="mt-4 flex items-center justify-between border-t border-white/10 pt-4 text-sm text-slate-400">
            <span>{t('adminTaskScheduler.pageSize')}</span>
            <div className="flex gap-2">
              <button
                className="rounded-lg border border-white/15 px-3 py-1.5 transition hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-40"
                disabled={previousOffset === null || isLoading}
                type="button"
                onClick={() => setOffset(previousOffset ?? 0)}
              >
                {t('adminTaskScheduler.previous')}
              </button>
              <button
                className="rounded-lg border border-white/15 px-3 py-1.5 transition hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-40"
                disabled={nextOffset === null || isLoading}
                type="button"
                onClick={() => setOffset(nextOffset ?? offset)}
              >
                {t('adminTaskScheduler.next')}
              </button>
            </div>
          </div>
        </div>

        <TaskDetails task={selectedTask} />
      </div>
    </section>
  );
}

async function loadTaskPage(
  apiBaseUrl: string,
  offset: number,
  sortField: TaskSortField,
  sortDirection: SortDirection,
) {
  return loadAdminSchedulerTasks(apiBaseUrl, {
    limit: PAGE_SIZE,
    offset,
    sortField,
    sortDirection,
  });
}

function TaskDetails({ task }: { task: ScheduledTask | null }) {
  const { t } = useTranslation();
  if (!task) {
    return (
      <aside className="rounded-3xl border border-dashed border-white/15 bg-slate-950/50 p-6 text-sm text-slate-400">
        {t('adminTaskScheduler.selectTask')}
      </aside>
    );
  }

  return (
    <aside className="space-y-4 rounded-3xl border border-white/10 bg-slate-950/70 p-6">
      <div>
        <p className="text-xs font-semibold uppercase tracking-[0.24em] text-cyan-300">{t('adminTaskScheduler.details')}</p>
        <h3 className="mt-2 break-all text-lg font-bold text-white">{task.id}</h3>
      </div>
      <dl className="grid gap-3 text-sm">
        <Detail label="Status" value={task.status} />
        <Detail label="Task type" value={task.taskType} />
        <Detail label="User" value={task.userId} />
        <Detail label="Attempts" value={String(task.attemptCount)} />
        <Detail label="Created" value={formatEpoch(task.createdAtEpochSeconds)} />
        <Detail label="Updated" value={formatEpoch(task.updatedAtEpochSeconds)} />
        <Detail label="Worker" value={task.claimedBy ?? '-'} />
        <Detail label="Next attempt" value={formatEpoch(task.nextAttemptAtEpochSeconds)} />
        <Detail label="Started" value={formatOptionalEpoch(task.startedAtEpochSeconds)} />
        <Detail label="Finished" value={formatOptionalEpoch(task.finishedAtEpochSeconds)} />
        <Detail label="Lease expires" value={formatOptionalEpoch(task.leaseExpiresAtEpochSeconds)} />
        <Detail label="Last heartbeat" value={formatOptionalEpoch(task.lastHeartbeatAtEpochSeconds)} />
        <Detail label="Timed out" value={formatOptionalEpoch(task.timedOutAtEpochSeconds)} />
        <Detail label="Execution timeout" value={`${task.executionTimeoutSeconds}s`} />
        <Detail label="Leader only" value={task.leaderOnly ? 'yes' : 'no'} />
        <Detail label="Dedupe" value={task.dedupeKey} />
      </dl>
      {task.errorMessage && (
        <div className="rounded-2xl border border-rose-400/30 bg-rose-500/10 p-3 text-sm text-rose-100">
          {task.errorMessage}
        </div>
      )}
      <JsonBlock title="Payload" value={task.payload} />
      <JsonBlock title="Checkpoint" value={task.checkpoint} />
    </aside>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
      <dt className="text-xs uppercase tracking-wide text-slate-500">{label}</dt>
      <dd className="mt-1 break-all font-medium text-slate-100">{value}</dd>
    </div>
  );
}

function JsonBlock({ title, value }: { title: string; value: unknown }) {
  return (
    <div>
      <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-slate-500">{title}</p>
      <pre className="max-h-72 overflow-auto rounded-2xl border border-white/10 bg-black/40 p-3 text-xs text-slate-200">
        {JSON.stringify(value, null, 2)}
      </pre>
    </div>
  );
}

function StatusBadge({ status }: { status: ScheduledTask['status'] }) {
  return (
    <span className={`inline-flex rounded-full px-2.5 py-1 text-xs font-bold uppercase tracking-wide ${badgeClassName(status)}`}>
      {status.replace('_', ' ')}
    </span>
  );
}

function canRetry(task: ScheduledTask) {
  return task.status === 'failed' || task.status === 'timed_out';
}

function rowClassName(task: ScheduledTask) {
  switch (task.status) {
    case 'completed':
      return 'bg-emerald-500/10 text-emerald-50';
    case 'timed_out':
      return 'bg-amber-500/10 text-amber-50';
    case 'failed':
      return 'bg-rose-500/10 text-rose-50';
    default:
      return 'bg-white/[0.035] text-slate-100';
  }
}

function badgeClassName(status: ScheduledTask['status']) {
  switch (status) {
    case 'completed':
      return 'bg-emerald-400/15 text-emerald-200';
    case 'timed_out':
      return 'bg-amber-400/15 text-amber-100';
    case 'failed':
      return 'bg-rose-400/15 text-rose-100';
    default:
      return 'bg-slate-400/10 text-slate-200';
  }
}

function formatOptionalEpoch(value: number | null) {
  return value === null ? '-' : formatEpoch(value);
}

function formatEpoch(value: number) {
  return new Date(value * 1000).toLocaleString();
}
