import { useCallback, useMemo } from 'react';

import { useApiBaseUrl } from '../../lib/apiBaseUrl';
import { get, post } from '../../lib/httpClient';
import {
  scheduledTaskSchema,
  sortDirectionSchema,
  taskListPageSchema,
  taskSortFieldSchema,
  type ScheduledTask,
  type TaskListPage,
  type TaskListParams,
} from './types';

function buildListPath(params: TaskListParams) {
  validateTaskListParams(params);
  const query = new URLSearchParams({
    limit: String(params.limit),
    offset: String(params.offset),
    sortField: params.sortField,
    sortDirection: params.sortDirection,
  });
  return `/api/admin/task-scheduler/tasks?${query.toString()}`;
}

export async function loadAdminSchedulerTasks(
  apiBaseUrl: string,
  params: TaskListParams,
): Promise<TaskListPage> {
  const data = await get(apiBaseUrl, buildListPath(params));
  return taskListPageSchema.parse(data);
}

export async function loadAdminSchedulerTask(
  apiBaseUrl: string,
  taskId: string,
): Promise<ScheduledTask> {
  const data = await get(apiBaseUrl, `/api/admin/task-scheduler/tasks/${encodeURIComponent(taskId)}`);
  return scheduledTaskSchema.parse(data);
}

export async function retryAdminSchedulerTask(
  apiBaseUrl: string,
  taskId: string,
): Promise<ScheduledTask> {
  const data = await post<undefined, unknown>(
    apiBaseUrl,
    `/api/admin/task-scheduler/tasks/${encodeURIComponent(taskId)}/retry`,
  );
  return scheduledTaskSchema.parse(data);
}

export function useAdminTaskSchedulerApi() {
  const apiBaseUrl = useApiBaseUrl();

  const getTasks = useCallback(
    async (params: TaskListParams) => loadAdminSchedulerTasks(apiBaseUrl, params),
    [apiBaseUrl],
  );
  const getTask = useCallback(
    async (taskId: string) => loadAdminSchedulerTask(apiBaseUrl, taskId),
    [apiBaseUrl],
  );
  const retryTask = useCallback(
    async (taskId: string) => retryAdminSchedulerTask(apiBaseUrl, taskId),
    [apiBaseUrl],
  );

  return useMemo(() => ({
    loadAdminSchedulerTasks: getTasks,
    loadAdminSchedulerTask: getTask,
    retryAdminSchedulerTask: retryTask,
  }), [getTask, getTasks, retryTask]);
}

function validateTaskListParams(params: TaskListParams) {
  if (!Number.isInteger(params.limit) || params.limit < 1 || params.limit > 20) {
    throw new Error('Invalid task list limit');
  }
  if (!Number.isInteger(params.offset) || params.offset < 0) {
    throw new Error('Invalid task list offset');
  }
  if (!taskSortFieldSchema.safeParse(params.sortField).success) {
    throw new Error('Invalid task list sort field');
  }
  if (!sortDirectionSchema.safeParse(params.sortDirection).success) {
    throw new Error('Invalid task list sort direction');
  }
}
