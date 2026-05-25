import { useEffect, useRef, useState } from 'react';
import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { useAdminTaskSchedulerApi } from '../features/admin-task-scheduler/api';
import { useMediaQuery } from '../lib/useMediaQuery';
import type {
  ScheduledTask,
  SortDirection,
  TaskSortField,
} from '../features/admin-task-scheduler/types';

const PAGE_SIZE = 20;

const columns: Array<{ field: TaskSortField; labelKey: string }> = [
  { field: 'id', labelKey: 'adminTaskScheduler.columns.id' },
  { field: 'createdAt', labelKey: 'adminTaskScheduler.columns.createdAt' },
  { field: 'status', labelKey: 'adminTaskScheduler.columns.status' },
  { field: 'taskType', labelKey: 'adminTaskScheduler.columns.taskType' },
  { field: 'userId', labelKey: 'adminTaskScheduler.columns.userId' },
  { field: 'attemptCount', labelKey: 'adminTaskScheduler.columns.attemptCount' },
  { field: 'nextAttemptAt', labelKey: 'adminTaskScheduler.columns.nextAttemptAt' },
  { field: 'startedAt', labelKey: 'adminTaskScheduler.columns.startedAt' },
  { field: 'finishedAt', labelKey: 'adminTaskScheduler.columns.finishedAt' },
  { field: 'updatedAt', labelKey: 'adminTaskScheduler.columns.updatedAt' },
  { field: 'claimedBy', labelKey: 'adminTaskScheduler.columns.claimedBy' },
  { field: 'leaseExpiresAt', labelKey: 'adminTaskScheduler.columns.leaseExpiresAt' },
  { field: 'lastHeartbeatAt', labelKey: 'adminTaskScheduler.columns.lastHeartbeatAt' },
  { field: 'executionTimeout', labelKey: 'adminTaskScheduler.columns.executionTimeout' },
  { field: 'timedOutAt', labelKey: 'adminTaskScheduler.columns.timedOutAt' },
  { field: 'leaderOnly', labelKey: 'adminTaskScheduler.columns.leaderOnly' },
  { field: 'errorMessage', labelKey: 'adminTaskScheduler.columns.errorMessage' },
  { field: 'dedupeKey', labelKey: 'adminTaskScheduler.columns.dedupeKey' },
];

export function AdminTaskSchedulerPage() {
  const { t } = useTranslation();
  const isMobile = useMediaQuery('(max-width: 767px)');
  const {
    loadAdminSchedulerTask,
    loadAdminSchedulerTasks,
    retryAdminSchedulerTask,
  } = useAdminTaskSchedulerApi();
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
  const selectionRequestIdRef = useRef(0);

  const loadTaskPage = async (
    pageOffset: number,
    pageSortField: TaskSortField,
    pageSortDirection: SortDirection,
  ) => loadAdminSchedulerTasks({
    limit: PAGE_SIZE,
    offset: pageOffset,
    sortField: pageSortField,
    sortDirection: pageSortDirection,
  });

  useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    void loadTaskPage(offset, sortField, sortDirection)
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
  }, [loadAdminSchedulerTasks, offset, sortField, sortDirection]);

  const refresh = async () => {
    setIsRefreshing(true);
    try {
      const [page, detail] = await Promise.all([
        loadTaskPage(offset, sortField, sortDirection),
        selectedTaskId ? loadAdminSchedulerTask(selectedTaskId).catch(() => null) : Promise.resolve(null),
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
    selectionRequestIdRef.current += 1;
    const requestId = selectionRequestIdRef.current;
    setSelectedTaskId(task.id);
    setSelectedTask(task);
    try {
      const detail = await loadAdminSchedulerTask(task.id);
      if (selectionRequestIdRef.current !== requestId) {
        return;
      }
      setSelectedTask(detail);
    } catch {
      if (selectionRequestIdRef.current !== requestId) {
        return;
      }
      setSelectedTask(task);
    }
  };

  const retryTask = async (taskId: string) => {
    setRetryingTaskId(taskId);
    try {
      const task = await retryAdminSchedulerTask(taskId);
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
          {isMobile ? (
            <div className="space-y-3">
              {tasks.map((task) => (
                <div
                  key={task.id}
                  className={`${rowClassName(task)} w-full rounded-2xl border px-4 py-4 transition hover:brightness-125`}
                >
                  <button
                    type="button"
                    aria-label={t('adminTaskScheduler.taskRowLabel', { id: task.id })}
                    className="w-full text-left"
                    onClick={() => {
                      void selectTask(task);
                    }}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <p className="truncate text-sm font-bold text-white">{task.id}</p>
                        <p className="mt-1 text-xs text-slate-400">{task.taskType}</p>
                      </div>
                      <StatusBadge status={task.status} />
                    </div>
                    <div className="mt-3 grid grid-cols-2 gap-3 text-xs text-slate-300">
                      <MobileTaskMeta label={t('adminTaskScheduler.columns.createdAt')} value={formatEpoch(task.createdAtEpochSeconds)} />
                      <MobileTaskMeta label={t('adminTaskScheduler.columns.updatedAt')} value={formatEpoch(task.updatedAtEpochSeconds)} />
                      <MobileTaskMeta label={t('adminTaskScheduler.columns.userId')} value={task.userId} />
                      <MobileTaskMeta label={t('adminTaskScheduler.columns.attemptCount')} value={String(task.attemptCount)} />
                    </div>
                  </button>
                  {canRetry(task) ? (
                    <div className="mt-4">
                      <button
                        className="rounded-lg border border-white/20 px-3 py-2 text-xs font-semibold text-white transition hover:bg-white/10 disabled:opacity-50"
                        disabled={retryingTaskId === task.id}
                        type="button"
                        onClick={(event) => {
                          event.stopPropagation();
                          void retryTask(task.id);
                        }}
                      >
                        {retryingTaskId === task.id ? t('adminTaskScheduler.retrying') : t('adminTaskScheduler.retry')}
                      </button>
                    </div>
                  ) : null}
                </div>
              ))}
            </div>
          ) : (
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
                          {t(column.labelKey)}{sortField === column.field ? ` ${sortDirection === 'asc' ? '↑' : '↓'}` : ''}
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
                      aria-label={t('adminTaskScheduler.taskRowLabel', { id: task.id })}
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
                      <td className="px-3 py-3 text-slate-300">{t(task.leaderOnly ? 'adminTaskScheduler.yes' : 'adminTaskScheduler.no')}</td>
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
          )}
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

function MobileTaskMeta({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-[10px] uppercase tracking-[0.18em] text-slate-500">{label}</p>
      <p className="mt-1 break-all text-sm text-slate-200">{value}</p>
    </div>
  );
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
        <Detail label={t('adminTaskScheduler.columns.status')} value={t(`adminTaskScheduler.statuses.${task.status}`)} />
        <Detail label={t('adminTaskScheduler.columns.taskType')} value={task.taskType} />
        <Detail label={t('adminTaskScheduler.columns.userId')} value={task.userId} />
        <Detail label={t('adminTaskScheduler.columns.attemptCount')} value={String(task.attemptCount)} />
        <Detail label={t('adminTaskScheduler.columns.createdAt')} value={formatEpoch(task.createdAtEpochSeconds)} />
        <Detail label={t('adminTaskScheduler.columns.updatedAt')} value={formatEpoch(task.updatedAtEpochSeconds)} />
        <Detail label={t('adminTaskScheduler.columns.claimedBy')} value={task.claimedBy ?? '-'} />
        <Detail label={t('adminTaskScheduler.columns.nextAttemptAt')} value={formatEpoch(task.nextAttemptAtEpochSeconds)} />
        <Detail label={t('adminTaskScheduler.columns.startedAt')} value={formatOptionalEpoch(task.startedAtEpochSeconds)} />
        <Detail label={t('adminTaskScheduler.columns.finishedAt')} value={formatOptionalEpoch(task.finishedAtEpochSeconds)} />
        <Detail label={t('adminTaskScheduler.columns.leaseExpiresAt')} value={formatOptionalEpoch(task.leaseExpiresAtEpochSeconds)} />
        <Detail label={t('adminTaskScheduler.columns.lastHeartbeatAt')} value={formatOptionalEpoch(task.lastHeartbeatAtEpochSeconds)} />
        <Detail label={t('adminTaskScheduler.columns.timedOutAt')} value={formatOptionalEpoch(task.timedOutAtEpochSeconds)} />
        <Detail label={t('adminTaskScheduler.columns.executionTimeout')} value={`${task.executionTimeoutSeconds}s`} />
        <Detail label={t('adminTaskScheduler.columns.leaderOnly')} value={t(task.leaderOnly ? 'adminTaskScheduler.yes' : 'adminTaskScheduler.no')} />
        <Detail label={t('adminTaskScheduler.columns.dedupeKey')} value={task.dedupeKey} />
      </dl>
      {task.errorMessage && (
        <div className="rounded-2xl border border-rose-400/30 bg-rose-500/10 p-3 text-sm text-rose-100">
          {task.errorMessage}
        </div>
      )}
      <JsonBlock title={t('adminTaskScheduler.payload')} value={task.payload} />
      <JsonBlock title={t('adminTaskScheduler.checkpoint')} value={task.checkpoint} />
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
  const { t } = useTranslation();

  return (
    <span className={`inline-flex rounded-full px-2.5 py-1 text-xs font-bold uppercase tracking-wide ${badgeClassName(status)}`}>
      {t(`adminTaskScheduler.statuses.${status}`)}
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
