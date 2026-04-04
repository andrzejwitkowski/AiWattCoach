import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import {
  createWorkoutSummary,
  getWorkoutSummary,
  reopenWorkoutSummary,
  saveWorkoutSummary,
  updateWorkoutSummaryRpe,
} from '../api/workoutSummary';
import {
  clientWsMessageSchema,
  serverWsMessageSchema,
  type ConversationMessage,
  type WorkoutSummary,
} from '../types';

type UseCoachChatOptions = {
  apiBaseUrl: string;
  workoutId: string | null;
};

type UseCoachChatResult = {
  summary: WorkoutSummary | null;
  messages: ConversationMessage[];
  draftRpe: number | null;
  isLoading: boolean;
  isSaving: boolean;
  isConnected: boolean;
  isCoachTyping: boolean;
  error: string | null;
  hasConversation: boolean;
  isSaved: boolean;
  setDraftRpe: (rpe: number) => void;
  sendMessage: (content: string) => Promise<boolean>;
  saveSummary: () => Promise<WorkoutSummary | null>;
  reopenSummary: () => Promise<WorkoutSummary | null>;
};

type PendingSocketState = {
  workoutId: string;
  socket: WebSocket;
  promise: Promise<WebSocket>;
};

class StaleWorkoutSelectionError extends Error {
  constructor() {
    super('Workout selection changed before the request completed.');
  }
}

function buildProtocol(protocol: string): 'ws:' | 'wss:' {
  return protocol === 'https:' ? 'wss:' : 'ws:';
}

export function buildWorkoutSummaryWebSocketUrl(apiBaseUrl: string, workoutId: string): string {
  const path = `/api/workout-summaries/${workoutId}/ws`;

  if (!apiBaseUrl) {
    return `${buildProtocol(window.location.protocol)}//${window.location.host}${path}`;
  }

  if (apiBaseUrl.startsWith('/')) {
    return `${buildProtocol(window.location.protocol)}//${window.location.host}${apiBaseUrl}${path}`;
  }

  const url = new URL(apiBaseUrl);
  const normalizedBasePath = url.pathname.endsWith('/')
    ? url.pathname.slice(0, -1)
    : url.pathname;
  url.pathname = `${normalizedBasePath}${path}`;
  url.protocol = buildProtocol(url.protocol);
  return url.toString();
}

function temporaryMessage(content: string): ConversationMessage {
  return {
    id: `temp-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    role: 'user',
    content,
    createdAtEpochSeconds: Math.floor(Date.now() / 1000),
  };
}

export function useCoachChat({ apiBaseUrl, workoutId }: UseCoachChatOptions): UseCoachChatResult {
  const [summary, setSummary] = useState<WorkoutSummary | null>(null);
  const [messages, setMessages] = useState<ConversationMessage[]>([]);
  const [draftRpe, setDraftRpe] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isConnected, setIsConnected] = useState(false);
  const [isCoachTyping, setIsCoachTyping] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const socketWorkoutIdRef = useRef<string | null>(null);
  const pendingSocketRef = useRef<PendingSocketState | null>(null);
  const currentWorkoutIdRef = useRef<string | null>(workoutId);
  const savingRequestIdRef = useRef(0);
  const localSystemMessageIdRef = useRef(0);

  useEffect(() => {
    currentWorkoutIdRef.current = workoutId;
    savingRequestIdRef.current += 1;
    setIsSaving(false);
  }, [workoutId]);

  const assertCurrentWorkout = useCallback((expectedWorkoutId: string) => {
    if (currentWorkoutIdRef.current !== expectedWorkoutId) {
      throw new StaleWorkoutSelectionError();
    }
  }, []);

  const applySummaryState = useCallback((nextSummary: WorkoutSummary, expectedWorkoutId: string) => {
    assertCurrentWorkout(expectedWorkoutId);
    setSummary(nextSummary);
    setMessages(nextSummary.messages);
    setDraftRpe(nextSummary.rpe);
  }, [assertCurrentWorkout]);

  const clearSummaryState = useCallback((expectedWorkoutId: string) => {
    assertCurrentWorkout(expectedWorkoutId);
    setSummary(null);
    setMessages([]);
    setDraftRpe(null);
  }, [assertCurrentWorkout]);

  const handleSetDraftRpe = useCallback((rpe: number) => {
    setDraftRpe(rpe);
    setError(null);
  }, []);

  const closeSocket = useCallback(() => {
    const pendingSocket = pendingSocketRef.current;
    pendingSocketRef.current = null;
    pendingSocket?.socket.close();

    if (socketRef.current) {
      socketRef.current.close();
      socketRef.current = null;
    }

    socketWorkoutIdRef.current = null;

    setIsConnected(false);
    setIsCoachTyping(false);
  }, []);

  const ensureSummaryExists = useCallback(async (): Promise<WorkoutSummary> => {
    if (!workoutId) {
      throw new Error('No workout selected.');
    }

    const requestedWorkoutId = workoutId;

    if (summary && summary.workoutId === requestedWorkoutId) {
      return summary;
    }

    try {
      const created = await createWorkoutSummary(apiBaseUrl, requestedWorkoutId);
      applySummaryState(created, requestedWorkoutId);
      return created;
    } catch (createError) {
      if (createError instanceof AuthenticationError) {
        throw createError;
      }

      if (createError instanceof HttpError && createError.status === 409) {
        const existing = await getWorkoutSummary(apiBaseUrl, requestedWorkoutId);
        applySummaryState(existing, requestedWorkoutId);
        return existing;
      }

      throw createError;
    }
  }, [apiBaseUrl, applySummaryState, workoutId, summary]);

  const connectSocket = useCallback(async (currentWorkoutId: string) => {
    if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
      if (socketWorkoutIdRef.current === currentWorkoutId) {
        return socketRef.current;
      }

      socketRef.current.close();
      socketRef.current = null;
      socketWorkoutIdRef.current = null;
      setIsConnected(false);
      setIsCoachTyping(false);
    }

    if (pendingSocketRef.current && pendingSocketRef.current.workoutId !== currentWorkoutId) {
      pendingSocketRef.current.socket.close();
      pendingSocketRef.current = null;
    }

    if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
      return socketRef.current;
    }

    if (pendingSocketRef.current?.workoutId === currentWorkoutId) {
      return pendingSocketRef.current.promise;
    }

    setError(null);

    const socket = new WebSocket(buildWorkoutSummaryWebSocketUrl(apiBaseUrl, currentWorkoutId));

    const socketPromise = new Promise<WebSocket>((resolve, reject) => {
      socket.addEventListener('open', () => {
        if (pendingSocketRef.current?.socket !== socket || pendingSocketRef.current?.workoutId !== currentWorkoutId) {
          socket.close();
          reject(new Error('WebSocket connection no longer needed'));
          return;
        }

        socketRef.current = socket;
        socketWorkoutIdRef.current = currentWorkoutId;
        setIsConnected(true);
        resolve(socket);
      }, { once: true });

      socket.addEventListener('message', (messageEvent) => {
        try {
          const parsed = serverWsMessageSchema.parse(JSON.parse(messageEvent.data as string));

          if (currentWorkoutIdRef.current !== currentWorkoutId) {
            return;
          }

          if (parsed.type === 'coach_typing') {
            setIsCoachTyping(true);
            return;
          }

          if (parsed.type === 'coach_message') {
            setSummary(parsed.summary);
            setMessages(parsed.summary.messages);
            setDraftRpe(parsed.summary.rpe);
            setIsCoachTyping(false);
            return;
          }

          if (parsed.type === 'system_message') {
            localSystemMessageIdRef.current += 1;
            setMessages((current) => [
              ...current,
              {
                id: `system-${localSystemMessageIdRef.current}`,
                role: 'system',
                content: parsed.content,
                createdAtEpochSeconds: Math.floor(Date.now() / 1000),
              },
            ]);
            setIsCoachTyping(false);
            return;
          }

          setError(parsed.error);
          setIsCoachTyping(false);
        } catch {
          setError('Received an invalid coach response.');
          setIsCoachTyping(false);
        }
      });

      socket.addEventListener('close', () => {
        if (socketRef.current === socket) {
          socketRef.current = null;
          socketWorkoutIdRef.current = null;
          setIsConnected(false);
        }
        if (pendingSocketRef.current?.socket === socket) {
          pendingSocketRef.current = null;
        }
        setIsCoachTyping(false);
      });

      socket.addEventListener('error', () => {
        if (pendingSocketRef.current?.socket === socket) {
          pendingSocketRef.current = null;
        }
        if (socketRef.current === socket) {
          socketRef.current = null;
          socketWorkoutIdRef.current = null;
        }
        setError('Unable to connect to the coach chat right now.');
        setIsConnected(false);
        setIsCoachTyping(false);
        reject(new Error('WebSocket connection failed'));
      }, { once: true });
    });

    pendingSocketRef.current = { workoutId: currentWorkoutId, socket, promise: socketPromise };

    try {
      return await socketPromise;
    } finally {
      if (pendingSocketRef.current?.workoutId === currentWorkoutId) {
        pendingSocketRef.current = null;
      }
    }
  }, [apiBaseUrl]);

  useEffect(() => {
    closeSocket();
    setSummary(null);
    setMessages([]);
    setDraftRpe(null);
    setError(null);

    if (!workoutId) {
      setIsLoading(false);
      return;
    }

    let cancelled = false;

    const loadSummary = async () => {
      setIsLoading(true);

      try {
        const loadedSummary = await getWorkoutSummary(apiBaseUrl, workoutId);

        if (cancelled) {
          return;
        }

        setSummary(loadedSummary);
        setMessages(loadedSummary.messages);
        setDraftRpe(loadedSummary.rpe);
        await connectSocket(workoutId);
      } catch (loadError) {
        if (cancelled) {
          return;
        }

        if (loadError instanceof AuthenticationError) {
          window.location.href = '/';
          return;
        }

        if (loadError instanceof HttpError && loadError.status === 404) {
          clearSummaryState(workoutId);
          return;
        }

        setError(loadError instanceof Error ? loadError.message : 'Unknown error');
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    };

    void loadSummary();

    return () => {
      cancelled = true;
      closeSocket();
    };
  }, [apiBaseUrl, clearSummaryState, closeSocket, connectSocket, workoutId]);

  const saveSummary = useCallback(async () => {
    if (!workoutId) {
      return null;
    }

    const requestedWorkoutId = workoutId;
    const requestId = savingRequestIdRef.current + 1;
    savingRequestIdRef.current = requestId;

    setIsSaving(true);
    setError(null);

    try {
      let nextSummary = summary;

      if (!nextSummary || nextSummary.workoutId !== requestedWorkoutId) {
        nextSummary = await ensureSummaryExists();
      }

      assertCurrentWorkout(requestedWorkoutId);

      if (draftRpe !== null && nextSummary.rpe !== draftRpe) {
        nextSummary = await updateWorkoutSummaryRpe(apiBaseUrl, requestedWorkoutId, draftRpe);
        applySummaryState(nextSummary, requestedWorkoutId);
      }

      nextSummary = await saveWorkoutSummary(apiBaseUrl, requestedWorkoutId);

      applySummaryState(nextSummary, requestedWorkoutId);
      return nextSummary;
    } catch (saveError) {
      if (saveError instanceof StaleWorkoutSelectionError) {
        return null;
      }

      if (saveError instanceof AuthenticationError) {
        window.location.href = '/';
        return null;
      }

      setError(saveError instanceof Error ? saveError.message : 'Unable to save this workout summary.');
      return null;
    } finally {
      if (savingRequestIdRef.current === requestId && currentWorkoutIdRef.current === requestedWorkoutId) {
        setIsSaving(false);
      }
    }
  }, [apiBaseUrl, applySummaryState, assertCurrentWorkout, draftRpe, ensureSummaryExists, workoutId, summary]);

  const reopenSummary = useCallback(async () => {
    if (!workoutId) {
      return null;
    }

    const requestedWorkoutId = workoutId;
    const requestId = savingRequestIdRef.current + 1;
    savingRequestIdRef.current = requestId;

    setIsSaving(true);
    setError(null);

    try {
      const nextSummary = await reopenWorkoutSummary(apiBaseUrl, requestedWorkoutId);
      applySummaryState(nextSummary, requestedWorkoutId);
      return nextSummary;
    } catch (saveError) {
      if (saveError instanceof StaleWorkoutSelectionError) {
        return null;
      }

      if (saveError instanceof AuthenticationError) {
        window.location.href = '/';
        return null;
      }

      setError(saveError instanceof Error ? saveError.message : 'Unable to reopen this workout summary.');
      return null;
    } finally {
      if (savingRequestIdRef.current === requestId && currentWorkoutIdRef.current === requestedWorkoutId) {
        setIsSaving(false);
      }
    }
  }, [apiBaseUrl, applySummaryState, workoutId]);

  const sendMessage = useCallback(async (content: string) => {
    const trimmed = content.trim();

    if (!trimmed || !workoutId) {
      return false;
    }

    const requestedWorkoutId = workoutId;

    if (draftRpe === null) {
      return false;
    }

    if (summary?.savedAtEpochSeconds != null) {
      setError('This summary is saved. Click Edit to continue coaching.');
      return false;
    }

    setError(null);

    try {
      let nextSummary = await ensureSummaryExists();
      assertCurrentWorkout(requestedWorkoutId);

      if (nextSummary.rpe !== draftRpe) {
        nextSummary = await updateWorkoutSummaryRpe(apiBaseUrl, requestedWorkoutId, draftRpe);
        applySummaryState(nextSummary, requestedWorkoutId);
      }

      const socket = await connectSocket(requestedWorkoutId);
      assertCurrentWorkout(requestedWorkoutId);
      const payload = clientWsMessageSchema.parse({ type: 'send_message', content: trimmed });
      socket.send(JSON.stringify(payload));
      setMessages((current) => {
        if (currentWorkoutIdRef.current !== requestedWorkoutId) {
          return current;
        }

        return [...current, temporaryMessage(trimmed)];
      });
      return true;
    } catch (sendError) {
      if (sendError instanceof StaleWorkoutSelectionError) {
        return false;
      }

      if (sendError instanceof AuthenticationError) {
        window.location.href = '/';
        return false;
      }

      setError(sendError instanceof Error ? sendError.message : 'Unable to send your message.');
      return false;
    }
  }, [apiBaseUrl, applySummaryState, assertCurrentWorkout, connectSocket, draftRpe, ensureSummaryExists, summary?.savedAtEpochSeconds, workoutId]);

  const hasConversation = useMemo(
    () => messages.some((message) => message.role === 'coach'),
    [messages],
  );

  const isSaved = summary?.savedAtEpochSeconds !== null && summary?.savedAtEpochSeconds !== undefined;

  return {
    summary,
    messages,
    draftRpe,
    isLoading,
    isSaving,
    isConnected,
    isCoachTyping,
    error,
    hasConversation,
    isSaved,
    setDraftRpe: handleSetDraftRpe,
    sendMessage,
    saveSummary,
    reopenSummary,
  };
}
