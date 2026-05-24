import { get, post } from '../../lib/httpClient';
import {
  scheduledTaskSchema,
  taskListPageSchema,
  type ScheduledTask,
  type TaskListPage,
  type TaskListParams,
} from './types';

function buildListPath(params: TaskListParams) {
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
