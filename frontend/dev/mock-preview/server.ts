import { createPreset, previewPresetNames, type PreviewPreset } from './presets';

const DEFAULT_PORT = 4010;

type SocketLike = {
  send: (message: string) => void;
};

type State = {
  preset: PreviewPreset;
  workoutSockets: Map<string, Set<SocketLike>>;
  calendarSockets: Map<string, Set<SocketLike>>;
};

function json(data: unknown, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      'content-type': 'application/json',
      'access-control-allow-origin': '*',
      'access-control-allow-headers': 'content-type, traceparent',
      'access-control-allow-methods': 'GET, POST, PATCH, PUT, DELETE, OPTIONS',
    },
  });
}

function notFound(message = 'Not found') {
  return json({ message }, 404);
}

function badRequest(message: string, code?: string) {
  return json(code ? { message, code } : { message }, 400);
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function pathname(url: URL) {
  return url.pathname.replace(/\/+$/, '') || '/';
}

function toLocalDateKey(value: Date) {
  const year = value.getFullYear();
  const month = String(value.getMonth() + 1).padStart(2, '0');
  const day = String(value.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function sortTasks(tasks: PreviewPreset['tasks'], sortField: string, sortDirection: string) {
  const direction = sortDirection === 'asc' ? 1 : -1;
  return [...tasks].sort((left, right) => {
    const leftValue = taskSortValue(left, sortField);
    const rightValue = taskSortValue(right, sortField);
    if (leftValue < rightValue) return -1 * direction;
    if (leftValue > rightValue) return 1 * direction;
    return left.id.localeCompare(right.id) * direction;
  });
}

function taskSortValue(task: PreviewPreset['tasks'][number], field: string) {
  switch (field) {
    case 'id': return task.id;
    case 'userId': return task.userId;
    case 'taskType': return task.taskType;
    case 'status': return task.status;
    case 'dedupeKey': return task.dedupeKey;
    case 'errorMessage': return task.errorMessage ?? '';
    case 'attemptCount': return task.attemptCount;
    case 'nextAttemptAt': return task.nextAttemptAtEpochSeconds;
    case 'claimedBy': return task.claimedBy ?? '';
    case 'leaseExpiresAt': return task.leaseExpiresAtEpochSeconds ?? -1;
    case 'lastHeartbeatAt': return task.lastHeartbeatAtEpochSeconds ?? -1;
    case 'executionTimeout': return task.executionTimeoutSeconds;
    case 'timedOutAt': return task.timedOutAtEpochSeconds ?? -1;
    case 'leaderOnly': return Number(task.leaderOnly);
    case 'updatedAt': return task.updatedAtEpochSeconds;
    case 'startedAt': return task.startedAtEpochSeconds ?? -1;
    case 'finishedAt': return task.finishedAtEpochSeconds ?? -1;
    case 'createdAt':
    default:
      return task.createdAtEpochSeconds;
  }
}

function listByDateRange<T extends { startDateLocal: string }>(items: T[], oldest: string, newest: string) {
  return items.filter((item) => {
    const dateKey = item.startDateLocal.slice(0, 10);
    return dateKey >= oldest && dateKey <= newest;
  });
}

async function parseBody(request: Request) {
  try {
    return await request.json();
  } catch {
    return null;
  }
}

function broadcastWorkout(state: State, workoutId: string, payload: unknown) {
  const sockets = state.workoutSockets.get(workoutId);
  if (!sockets) return;
  const message = JSON.stringify(payload);
  for (const socket of sockets) {
    socket.send(message);
  }
}

function broadcastCalendar(state: State, conversationId: string, payload: unknown) {
  const sockets = state.calendarSockets.get(conversationId);
  if (!sockets) return;
  const message = JSON.stringify(payload);
  for (const socket of sockets) {
    socket.send(message);
  }
}

function ensureWorkoutSummary(state: State, workoutId: string) {
  const existing = state.preset.workoutSummaries[workoutId];
  if (existing) {
    return existing;
  }

  const now = Math.floor(Date.now() / 1000);
  const created = {
    id: `summary-${workoutId}`,
    workoutId,
    rpe: null,
    messages: [],
    savedAtEpochSeconds: null,
    createdAtEpochSeconds: now,
    updatedAtEpochSeconds: now,
  };
  state.preset.workoutSummaries[workoutId] = created;
  return created;
}

function ensureCalendarConversation(state: State) {
  const currentId = state.preset.calendarCoach.currentConversationId;
  if (currentId) {
    return state.preset.calendarCoach.conversations[currentId];
  }

  const now = Math.floor(Date.now() / 1000);
  const conversationId = `calendar-conv-${now}`;
  const created = {
    conversation: {
      conversationId,
      surface: 'calendar' as const,
      status: 'active' as const,
      focus: 'overview' as const,
      createdAtEpochSeconds: now,
      updatedAtEpochSeconds: now,
    },
    messages: [],
  };
  state.preset.calendarCoach.currentConversationId = conversationId;
  state.preset.calendarCoach.conversations[conversationId] = created;
  return created;
}

function corsResponse() {
  return new Response(null, {
    status: 204,
    headers: {
      'access-control-allow-origin': '*',
      'access-control-allow-headers': 'content-type, traceparent',
      'access-control-allow-methods': 'GET, POST, PATCH, PUT, DELETE, OPTIONS',
    },
  });
}

function createServerState(): State {
  return {
    preset: createPreset(process.env.PREVIEW_PRESET),
    workoutSockets: new Map(),
    calendarSockets: new Map(),
  };
}

const state = createServerState();
const port = Number(process.env.PREVIEW_PORT ?? DEFAULT_PORT);

const server = Bun.serve({
  port,
  fetch(request, server) {
    const url = new URL(request.url);
    const path = pathname(url);

    if (request.method === 'OPTIONS') {
      return corsResponse();
    }

    const workoutWsMatch = path.match(/^\/api\/workout-summaries\/([^/]+)\/ws$/);
    if (workoutWsMatch) {
      if (server.upgrade(request, { data: { kind: 'workout', key: decodeURIComponent(workoutWsMatch[1] ?? '') } })) {
        return undefined;
      }
      return badRequest('WebSocket upgrade failed');
    }

    const calendarWsMatch = path.match(/^\/api\/calendar\/coach\/conversations\/([^/]+)\/ws$/);
    if (calendarWsMatch) {
      if (server.upgrade(request, { data: { kind: 'calendar', key: decodeURIComponent(calendarWsMatch[1] ?? '') } })) {
        return undefined;
      }
      return badRequest('WebSocket upgrade failed');
    }

    return handleRequest(state, request, url, path);
  },
  websocket: {
    open(ws) {
      const data = ws.data as { kind: 'workout' | 'calendar'; key: string };
      const store = data.kind === 'workout' ? state.workoutSockets : state.calendarSockets;
      const sockets = store.get(data.key) ?? new Set<SocketLike>();
      sockets.add(ws as unknown as SocketLike);
      store.set(data.key, sockets);
    },
    message(ws, message) {
      const data = ws.data as { kind: 'workout' | 'calendar'; key: string };
      const raw = typeof message === 'string' ? message : Buffer.from(message).toString('utf8');
      let parsed: unknown;

      try {
        parsed = JSON.parse(raw);
      } catch {
        ws.send(JSON.stringify({ type: 'error', error: 'Invalid JSON payload.' }));
        return;
      }

      if (data.kind === 'workout') {
        const payload = parsed as { type?: string; content?: string };
        if (payload.type !== 'send_message' || !payload.content?.trim()) {
          ws.send(JSON.stringify({ type: 'error', error: 'Invalid message payload.' }));
          return;
        }

        const summary = ensureWorkoutSummary(state, data.key);
        const now = Math.floor(Date.now() / 1000);
        const userMessage = {
          id: `msg-${data.key}-${now}-user`,
          role: 'user' as const,
          content: payload.content.trim(),
          createdAtEpochSeconds: now,
        };
        const coachMessage = {
          id: `msg-${data.key}-${now}-coach`,
          role: 'coach' as const,
          content: 'Preview coach reply: keep the first interval slightly calmer and preserve cadence through the midpoint.',
          createdAtEpochSeconds: now + 1,
        };
        summary.messages = [...summary.messages, userMessage, coachMessage];
        summary.updatedAtEpochSeconds = now + 1;
        broadcastWorkout(state, data.key, { type: 'coach_typing' });
        broadcastWorkout(state, data.key, { type: 'coach_message', message: coachMessage, summary: clone(summary) });
        return;
      }

      const payload = parsed as { type?: string; content?: string };
      if (payload.type !== 'send_message' || !payload.content?.trim()) {
        ws.send(JSON.stringify({ type: 'error', error: 'Invalid message payload.' }));
        return;
      }

      const conversation = state.preset.calendarCoach.conversations[data.key];
      if (!conversation) {
        ws.send(JSON.stringify({ type: 'error', error: 'Conversation not found.' }));
        return;
      }

      const now = Math.floor(Date.now() / 1000);
      const userMessage = {
        id: `calendar-msg-${now}-user`,
        role: 'user' as const,
        content: payload.content.trim(),
        createdAtEpochSeconds: now,
      };
      const toolMessage = {
        id: `calendar-msg-${now}-tool`,
        role: 'tool' as const,
        content: 'Updated Friday workout timing.',
        toolCall: {
          id: `tool-${now}`,
          name: 'update_planned_workout',
          argumentsJson: '{"date":"preview"}',
          argumentsPreview: 'Shifted workout timing by 15 min',
        },
        createdAtEpochSeconds: now + 1,
      };
      const coachMessage = {
        id: `calendar-msg-${now}-coach`,
        role: 'coach' as const,
        content: 'Preview calendar reply: the week stays balanced if you keep tomorrow easy and let Saturday absorb the main load.',
        createdAtEpochSeconds: now + 2,
      };
      conversation.messages = [...conversation.messages, userMessage, toolMessage, coachMessage];
      conversation.conversation.updatedAtEpochSeconds = now + 2;
      broadcastCalendar(state, data.key, { type: 'coach_thinking' });
      broadcastCalendar(state, data.key, { type: 'tool_message', message: toolMessage });
      broadcastCalendar(state, data.key, {
        type: 'coach_message',
        message: coachMessage,
        conversation: clone(conversation.conversation),
        messages: clone(conversation.messages),
      });
    },
    close(ws) {
      const data = ws.data as { kind: 'workout' | 'calendar'; key: string };
      const store = data.kind === 'workout' ? state.workoutSockets : state.calendarSockets;
      const sockets = store.get(data.key);
      if (!sockets) return;
      sockets.delete(ws as unknown as SocketLike);
      if (sockets.size === 0) {
        store.delete(data.key);
      }
    },
  },
});

console.log(`Mock preview API listening on http://127.0.0.1:${server.port}`);
console.log(`Preset: ${state.preset.name} (${state.preset.label})`);
console.log(`Available presets: ${previewPresetNames.join(', ')}`);

async function handleRequest(state: State, request: Request, url: URL, path: string) {
  if (path === '/health') {
    return json({ status: 'ok', service: 'AiWattCoach Preview Mock' });
  }

  if (path === '/ready') {
    return json({ status: 'ready', reason: null });
  }

  if (path === '/api/auth/me') {
    return json({ authenticated: true, user: clone(state.preset.currentUser) });
  }

  if (path === '/api/auth/google/start') {
    const returnTo = url.searchParams.get('returnTo') ?? '/calendar';
    return new Response(null, {
      status: 302,
      headers: {
        location: returnTo,
        'access-control-allow-origin': '*',
      },
    });
  }

  if (path === '/api/auth/wahoo/start') {
    const returnTo = url.searchParams.get('returnTo') ?? '/settings';
    return new Response(null, {
      status: 302,
      headers: {
        location: returnTo,
        'access-control-allow-origin': '*',
      },
    });
  }

  if (path === '/api/auth/logout') {
    return json({ success: true });
  }

  if (path === '/api/auth/whitelist' && request.method === 'POST') {
    return json({ success: true });
  }

  if (path === '/api/settings') {
    return json(clone(state.preset.settings));
  }

  if (path === '/api/settings/ai-agents' && request.method === 'PATCH') {
    const body = await parseBody(request) as Record<string, string | null> | null;
    const next = state.preset.settings.aiAgents;
    if (body) {
      if ('openaiApiKey' in body) {
        next.openaiApiKey = body.openaiApiKey ?? null;
        next.openaiApiKeySet = Boolean(body.openaiApiKey);
      }
      if ('selectedProvider' in body) {
        next.selectedProvider = (body.selectedProvider || null) as PreviewPreset['settings']['aiAgents']['selectedProvider'];
      }
      if ('selectedModel' in body) {
        next.selectedModel = body.selectedModel || null;
      }
    }
    return json({ success: true });
  }

  if (path === '/api/settings/ai-agents/test' && request.method === 'POST') {
    return json({
      connected: true,
      message: 'Preview AI connection looks healthy.',
      usedSavedApiKey: true,
      usedSavedProvider: true,
      usedSavedModel: true,
    });
  }

  if (path === '/api/settings/intervals' && request.method === 'PATCH') {
    const body = await parseBody(request) as Record<string, string | null> | null;
    if (body) {
      if ('apiKey' in body) {
        state.preset.settings.intervals.apiKey = body.apiKey ?? null;
        state.preset.settings.intervals.apiKeySet = Boolean(body.apiKey);
      }
      if ('athleteId' in body) {
        state.preset.settings.intervals.athleteId = body.athleteId ?? null;
      }
      state.preset.settings.intervals.connected = state.preset.settings.intervals.apiKeySet && Boolean(state.preset.settings.intervals.athleteId);
    }
    return json({ success: true });
  }

  if (path === '/api/settings/intervals/test' && request.method === 'POST') {
    return json({
      connected: true,
      message: 'Preview Intervals.icu credentials are valid.',
      usedSavedApiKey: true,
      usedSavedAthleteId: true,
      persistedStatusUpdated: false,
    });
  }

  if (path === '/api/settings/options' && request.method === 'PATCH') {
    const body = await parseBody(request) as { analyzeWithoutHeartRate?: boolean } | null;
    if (typeof body?.analyzeWithoutHeartRate === 'boolean') {
      state.preset.settings.options.analyzeWithoutHeartRate = body.analyzeWithoutHeartRate;
    }
    return json({ success: true });
  }

  if (path === '/api/settings/availability' && request.method === 'PATCH') {
    const body = await parseBody(request) as { days?: PreviewPreset['settings']['availability']['days'] } | null;
    if (body?.days) {
      state.preset.settings.availability.days = body.days;
      state.preset.settings.availability.configured = body.days.some((day) => day.available);
    }
    return json(clone(state.preset.settings));
  }

  if (path === '/api/settings/cycling' && request.method === 'PATCH') {
    const body = await parseBody(request) as Record<string, string | number | null> | null;
    if (body) {
      state.preset.settings.cycling = {
        ...state.preset.settings.cycling,
        ...body,
      } as PreviewPreset['settings']['cycling'];
    }
    return json({ success: true });
  }

  if (path === '/api/athlete-summary') {
    return json(clone(state.preset.athleteSummary));
  }

  if (path === '/api/athlete-summary/generate' && request.method === 'POST') {
    const now = Math.floor(Date.now() / 1000);
    state.preset.athleteSummary = {
      exists: true,
      stale: false,
      summaryText: 'Preview athlete summary refreshed. The athlete is carrying productive fatigue and should recover well with one lighter day before the next quality block.',
      generatedAtEpochSeconds: now,
      updatedAtEpochSeconds: now,
    };
    return json(clone(state.preset.athleteSummary));
  }

  if (path === '/api/dashboard/training-load') {
    const range = (url.searchParams.get('range') ?? '90d') as keyof PreviewPreset['dashboardByRange'];
    const report = state.preset.dashboardByRange[range] ?? state.preset.dashboardByRange['90d'];
    return json(clone(report));
  }

  if (path === '/api/intervals/events') {
    const oldest = url.searchParams.get('oldest') ?? '0000-00-00';
    const newest = url.searchParams.get('newest') ?? '9999-99-99';
    return json(clone(listByDateRange(state.preset.events, oldest, newest)));
  }

  if (path === '/api/calendar/events') {
    const oldest = url.searchParams.get('oldest') ?? '0000-00-00';
    const newest = url.searchParams.get('newest') ?? '9999-99-99';
    return json(clone(listByDateRange(state.preset.events, oldest, newest)));
  }

  if (path === '/api/calendar/labels') {
    return json({ labelsByDate: clone(state.preset.labelsByDate) });
  }

  const eventMatch = path.match(/^\/api\/intervals\/events\/(\d+)$/);
  if (eventMatch) {
    const id = Number(eventMatch[1]);
    const event = state.preset.events.find((item) => item.id === id || item.linkedIntervalsEventId === id);
    if (!event) return notFound('Event not found');
    return json(clone(event));
  }

  const activityMatch = path.match(/^\/api\/completed-workouts\/([^/]+)$/);
  if (activityMatch && !path.endsWith('/summary')) {
    const activityId = decodeURIComponent(activityMatch[1] ?? '');
    const activity = state.preset.activities.find((item) => item.id === activityId);
    if (!activity) return notFound('Activity not found');
    return json(clone(activity));
  }

  const activitySummaryMatch = path.match(/^\/api\/completed-workouts\/([^/]+)\/summary$/);
  if (activitySummaryMatch) {
    const activityId = decodeURIComponent(activitySummaryMatch[1] ?? '');
    const summary = state.preset.completedWorkoutSummaries[activityId];
    if (!summary) return notFound('Completed workout summary not found');
    return json(clone(summary));
  }

  if (path === '/api/completed-workouts') {
    const oldest = url.searchParams.get('oldest') ?? '0000-00-00';
    const newest = url.searchParams.get('newest') ?? '9999-99-99';
    return json(clone(listByDateRange(state.preset.activities, oldest, newest)));
  }

  if (path === '/api/workout-summaries') {
    const ids = (url.searchParams.get('workoutIds') ?? '').split(',').filter(Boolean);
    const summaries = ids.map((id) => state.preset.workoutSummaries[id]).filter(Boolean);
    return json(clone(summaries));
  }

  const workoutSummaryMatch = path.match(/^\/api\/workout-summaries\/([^/]+)$/);
  if (workoutSummaryMatch) {
    const workoutId = decodeURIComponent(workoutSummaryMatch[1] ?? '');
    if (request.method === 'POST') {
      const created = ensureWorkoutSummary(state, workoutId);
      return json(clone(created));
    }
    const summary = state.preset.workoutSummaries[workoutId];
    if (!summary) return notFound('Workout summary not found');
    return json(clone(summary));
  }

  const workoutRpeMatch = path.match(/^\/api\/workout-summaries\/([^/]+)\/rpe$/);
  if (workoutRpeMatch && request.method === 'PATCH') {
    const workoutId = decodeURIComponent(workoutRpeMatch[1] ?? '');
    const body = await parseBody(request) as { rpe?: number } | null;
    const summary = ensureWorkoutSummary(state, workoutId);
    summary.rpe = typeof body?.rpe === 'number' ? body.rpe : summary.rpe;
    summary.updatedAtEpochSeconds = Math.floor(Date.now() / 1000);
    return json(clone(summary));
  }

  const workoutStateMatch = path.match(/^\/api\/workout-summaries\/([^/]+)\/state$/);
  if (workoutStateMatch) {
    const workoutId = decodeURIComponent(workoutStateMatch[1] ?? '');
    const body = await parseBody(request) as { saved?: boolean } | null;
    const summary = ensureWorkoutSummary(state, workoutId);
    const now = Math.floor(Date.now() / 1000);
    const saved = request.method === 'POST' ? true : body?.saved === false ? false : true;
    summary.savedAtEpochSeconds = saved ? now : null;
    summary.updatedAtEpochSeconds = now;
    return json({
      summary: clone(summary),
      workflow: {
        recapStatus: saved ? 'generated' : 'unchanged',
        planStatus: saved ? 'processing' : 'unchanged',
        messages: saved
          ? ['Preview workflow queued a background plan refresh.']
          : [],
      },
    });
  }

  if (path === '/api/calendar/coach/current') {
    const currentId = state.preset.calendarCoach.currentConversationId;
    if (!currentId) return notFound('No active calendar conversation');
    const conversation = state.preset.calendarCoach.conversations[currentId];
    return json({ conversation: clone(conversation.conversation), messages: clone(conversation.messages) });
  }

  if (path === '/api/calendar/coach/conversations' && request.method === 'POST') {
    const now = Math.floor(Date.now() / 1000);
    const conversationId = `calendar-conv-${now}`;
    const created = {
      conversation: {
        conversationId,
        surface: 'calendar' as const,
        status: 'active' as const,
        focus: 'overview' as const,
        createdAtEpochSeconds: now,
        updatedAtEpochSeconds: now,
      },
      messages: [],
    };
    state.preset.calendarCoach.currentConversationId = conversationId;
    state.preset.calendarCoach.conversations[conversationId] = created;
    return json(clone(created));
  }

  const calendarConversationMatch = path.match(/^\/api\/calendar\/coach\/conversations\/([^/]+)$/);
  if (calendarConversationMatch) {
    const conversationId = decodeURIComponent(calendarConversationMatch[1] ?? '');
    const conversation = state.preset.calendarCoach.conversations[conversationId];
    if (!conversation) return notFound('Calendar conversation not found');
    return json({ conversation: clone(conversation.conversation), messages: clone(conversation.messages) });
  }

  const calendarMessagesMatch = path.match(/^\/api\/calendar\/coach\/conversations\/([^/]+)\/messages$/);
  if (calendarMessagesMatch && request.method === 'POST') {
    const conversationId = decodeURIComponent(calendarMessagesMatch[1] ?? '');
    const conversation = state.preset.calendarCoach.conversations[conversationId];
    if (!conversation) {
      return notFound('Calendar conversation not found');
    }
    const body = await parseBody(request) as { content?: string } | null;
    if (!body?.content?.trim()) {
      return badRequest('Message content is required');
    }
    const now = Math.floor(Date.now() / 1000);
    const userMessage = {
      id: `calendar-msg-${now}-user`,
      role: 'user' as const,
      content: body.content.trim(),
      createdAtEpochSeconds: now,
    };
    const toolMessage = {
      id: `calendar-msg-${now}-tool`,
      role: 'tool' as const,
      content: 'Preview tool updated one planned workout.',
      toolCall: {
        id: `calendar-tool-${now}`,
        name: 'update_planned_workout',
        argumentsJson: '{"date":"preview"}',
        argumentsPreview: 'Workout moved later by 15 min',
      },
      createdAtEpochSeconds: now + 1,
    };
    const coachMessage = {
      id: `calendar-msg-${now}-coach`,
      role: 'coach' as const,
      content: 'Preview calendar reply: the updated placement keeps your week balanced and preserves Saturday quality.',
      createdAtEpochSeconds: now + 2,
    };
    conversation.messages = [...conversation.messages, userMessage, toolMessage, coachMessage];
    conversation.conversation.updatedAtEpochSeconds = now + 2;
    return json({
      conversation: clone(conversation.conversation),
      messages: clone(conversation.messages),
      userMessage: clone(userMessage),
      coachMessage: clone(coachMessage),
    });
  }

  if (path === '/api/admin/system-info') {
    return json({
      appName: 'AiWattCoach Preview Mock',
      mongoDatabase: `preview_${state.preset.name.replace(/-/g, '_')}`,
    });
  }

  if (path === '/api/admin/task-scheduler/tasks') {
    const limit = Number(url.searchParams.get('limit') ?? '20');
    const offset = Number(url.searchParams.get('offset') ?? '0');
    const sortField = url.searchParams.get('sortField') ?? 'createdAt';
    const sortDirection = url.searchParams.get('sortDirection') ?? 'desc';
    const items = sortTasks(state.preset.tasks, sortField, sortDirection);
    const pageItems = items.slice(offset, offset + limit);
    return json({
      items: clone(pageItems),
      nextOffset: offset + limit < items.length ? offset + limit : null,
      previousOffset: offset > 0 ? Math.max(0, offset - limit) : null,
      limit,
    });
  }

  const taskMatch = path.match(/^\/api\/admin\/task-scheduler\/tasks\/([^/]+)$/);
  if (taskMatch) {
    const taskId = decodeURIComponent(taskMatch[1] ?? '');
    const task = state.preset.tasks.find((item) => item.id === taskId);
    if (!task) return notFound('Task not found');
    return json(clone(task));
  }

  const retryTaskMatch = path.match(/^\/api\/admin\/task-scheduler\/tasks\/([^/]+)\/retry$/);
  if (retryTaskMatch && request.method === 'POST') {
    const taskId = decodeURIComponent(retryTaskMatch[1] ?? '');
    const task = state.preset.tasks.find((item) => item.id === taskId);
    if (!task) return notFound('Task not found');
    task.status = 'queued';
    task.errorMessage = null;
    task.updatedAtEpochSeconds = Math.floor(Date.now() / 1000);
    return json(clone(task));
  }

  if (path === '/api/races') {
    if (request.method === 'GET') {
      return json(clone(state.preset.races));
    }
    if (request.method === 'POST') {
      const body = await parseBody(request) as { date?: string; name?: string; distanceMeters?: number; discipline?: PreviewPreset['races'][number]['discipline']; priority?: PreviewPreset['races'][number]['priority'] } | null;
      if (!body?.date || !body.name || !body.distanceMeters || !body.discipline || !body.priority) {
        return badRequest('Missing required race fields');
      }
      const race = {
        raceId: `race-${Date.now()}`,
        date: body.date,
        name: body.name,
        distanceMeters: body.distanceMeters,
        discipline: body.discipline,
        priority: body.priority,
        syncStatus: 'pending' as const,
        linkedIntervalsEventId: null,
        lastError: null,
      };
      state.preset.races.push(race);
      return json(clone(race));
    }
  }

  const raceMatch = path.match(/^\/api\/races\/([^/]+)$/);
  if (raceMatch) {
    const raceId = decodeURIComponent(raceMatch[1] ?? '');
    const race = state.preset.races.find((item) => item.raceId === raceId);
    if (!race) return notFound('Race not found');
    if (request.method === 'GET') {
      return json(clone(race));
    }
    if (request.method === 'PUT') {
      const body = await parseBody(request) as { date?: string; name?: string; distanceMeters?: number; discipline?: PreviewPreset['races'][number]['discipline']; priority?: PreviewPreset['races'][number]['priority'] } | null;
      if (!body?.date || !body.name || !body.distanceMeters || !body.discipline || !body.priority) {
        return badRequest('Missing required race fields');
      }
      race.date = body.date;
      race.name = body.name;
      race.distanceMeters = body.distanceMeters;
      race.discipline = body.discipline;
      race.priority = body.priority;
      return json(clone(race));
    }
  }

  if (path === '/api/calendar/refresh' && request.method === 'POST') {
    return json({
      oldest: state.preset.events.map((event) => event.startDateLocal.slice(0, 10)).sort()[0] ?? 'n/a',
      newest: state.preset.events.map((event) => event.startDateLocal.slice(0, 10)).sort().at(-1) ?? 'n/a',
      rebuiltEntryCount: state.preset.events.length,
    });
  }

  const syncMatch = path.match(/^\/api\/calendar\/planned-workouts\/([^/]+)\/([^/]+)\/(intervals|wahoo)\/sync$/);
  if (syncMatch && request.method === 'POST') {
    const [, operationKeyRaw, date, provider] = syncMatch;
    const operationKey = decodeURIComponent(operationKeyRaw ?? '');
    const event = state.preset.events.find((item) => item.projectedWorkout?.operationKey === operationKey && item.projectedWorkout?.date === date);
    if (!event) {
      return notFound('Projected workout not found');
    }
    if (provider === 'wahoo') {
      const today = new Date();
      const maxDate = new Date(today);
      maxDate.setDate(maxDate.getDate() + 6);
      const todayKey = toLocalDateKey(today);
      const maxDateKey = toLocalDateKey(maxDate);
      if (date < todayKey || date > maxDateKey) {
        return badRequest('Only planned workouts scheduled between today and the next 6 days can sync to Wahoo', 'wahoo_window_out_of_range');
      }
    }
    event.syncStatus = 'synced';
    event.linkedIntervalsEventId = event.id;
    return json(clone(event));
  }

  if (path.endsWith('/download.fit')) {
    return new Response(new Uint8Array([0x46, 0x49, 0x54]), {
      status: 200,
      headers: {
        'content-type': 'application/octet-stream',
        'access-control-allow-origin': '*',
      },
    });
  }

  return notFound();
}
