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
  sendMessage: (content: string) => Promise<void>;
  saveSummary: () => Promise<WorkoutSummary | null>;
  reopenSummary: () => Promise<WorkoutSummary | null>;
};

type PendingSocketState = {
  workoutId: string;
  promise: Promise<WebSocket>;
};

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

  const url = new URL(path, apiBaseUrl.endsWith('/') ? apiBaseUrl : `${apiBaseUrl}/`);
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
  const pendingSocketRef = useRef<PendingSocketState | null>(null);

  const handleSetDraftRpe = useCallback((rpe: number) => {
    setDraftRpe(rpe);
    setError(null);
  }, []);

  const closeSocket = useCallback(() => {
    pendingSocketRef.current = null;

    if (socketRef.current) {
      socketRef.current.close();
      socketRef.current = null;
    }

    setIsConnected(false);
    setIsCoachTyping(false);
  }, []);

  const ensureSummaryExists = useCallback(async (): Promise<WorkoutSummary> => {
    if (!workoutId) {
      throw new Error('No workout selected.');
    }

    if (summary && summary.workoutId === workoutId) {
      return summary;
    }

    try {
      const created = await createWorkoutSummary(apiBaseUrl, workoutId);
      setSummary(created);
      setMessages(created.messages);
      setDraftRpe(created.rpe);
      return created;
    } catch (createError) {
      if (createError instanceof AuthenticationError) {
        throw createError;
      }

      if (createError instanceof HttpError && createError.status === 409) {
        const existing = await getWorkoutSummary(apiBaseUrl, workoutId);
        setSummary(existing);
        setMessages(existing.messages);
        setDraftRpe(existing.rpe);
        return existing;
      }

      throw createError;
    }
  }, [apiBaseUrl, workoutId, summary]);

  const connectSocket = useCallback(async (currentWorkoutId: string) => {
    if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
      return socketRef.current;
    }

    if (pendingSocketRef.current?.workoutId === currentWorkoutId) {
      return pendingSocketRef.current.promise;
    }

    const socketPromise = new Promise<WebSocket>((resolve, reject) => {
      const socket = new WebSocket(buildWorkoutSummaryWebSocketUrl(apiBaseUrl, currentWorkoutId));

      socket.addEventListener('open', () => {
        socketRef.current = socket;
        setIsConnected(true);
        resolve(socket);
      }, { once: true });

      socket.addEventListener('message', (messageEvent) => {
        try {
          const parsed = serverWsMessageSchema.parse(JSON.parse(messageEvent.data as string));

          if (parsed.type === 'coach_typing') {
            setIsCoachTyping(true);
            return;
          }

          if (parsed.type === 'coach_message') {
            setSummary(parsed.summary);
            setMessages(parsed.summary.messages);
            setDraftRpe((current) => current ?? parsed.summary.rpe);
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
        }
        setIsConnected(false);
        setIsCoachTyping(false);
      });

      socket.addEventListener('error', () => {
        setError('Unable to connect to the coach chat right now.');
        setIsConnected(false);
        setIsCoachTyping(false);
        reject(new Error('WebSocket connection failed'));
      }, { once: true });
    });

    pendingSocketRef.current = { workoutId: currentWorkoutId, promise: socketPromise };

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
          setSummary(null);
          setMessages([]);
          setDraftRpe(null);
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
  }, [apiBaseUrl, closeSocket, connectSocket, workoutId]);

  const saveSummary = useCallback(async () => {
    if (!workoutId) {
      return null;
    }

    setIsSaving(true);
    setError(null);

    try {
      let nextSummary = summary;

      if (!nextSummary || nextSummary.workoutId !== workoutId) {
        nextSummary = await ensureSummaryExists();
      }

      if (draftRpe !== null && nextSummary.rpe !== draftRpe) {
        nextSummary = await updateWorkoutSummaryRpe(apiBaseUrl, workoutId, draftRpe);
      }

      nextSummary = await saveWorkoutSummary(apiBaseUrl, workoutId);

      setSummary(nextSummary);
      setMessages(nextSummary.messages);
      return nextSummary;
    } catch (saveError) {
      if (saveError instanceof AuthenticationError) {
        window.location.href = '/';
        return null;
      }

      setError(saveError instanceof Error ? saveError.message : 'Unable to save this workout summary.');
      return null;
    } finally {
      setIsSaving(false);
    }
  }, [apiBaseUrl, draftRpe, ensureSummaryExists, workoutId, summary]);

  const reopenSummary = useCallback(async () => {
    if (!workoutId) {
      return null;
    }

    setIsSaving(true);
    setError(null);

    try {
      const nextSummary = await reopenWorkoutSummary(apiBaseUrl, workoutId);
      setSummary(nextSummary);
      setMessages(nextSummary.messages);
      setDraftRpe(nextSummary.rpe);
      return nextSummary;
    } catch (saveError) {
      if (saveError instanceof AuthenticationError) {
        window.location.href = '/';
        return null;
      }

      setError(saveError instanceof Error ? saveError.message : 'Unable to reopen this workout summary.');
      return null;
    } finally {
      setIsSaving(false);
    }
  }, [apiBaseUrl, workoutId]);

  const sendMessage = useCallback(async (content: string) => {
    const trimmed = content.trim();

    if (!trimmed || !workoutId) {
      return;
    }

    if (draftRpe === null) {
      return;
    }

    if (summary?.savedAtEpochSeconds) {
      setError('This summary is saved. Click Edit to continue coaching.');
      return;
    }

    setError(null);

    try {
      let nextSummary = await ensureSummaryExists();

      if (nextSummary.rpe !== draftRpe) {
        nextSummary = await updateWorkoutSummaryRpe(apiBaseUrl, workoutId, draftRpe);
        setSummary(nextSummary);
        setMessages(nextSummary.messages);
        setDraftRpe(nextSummary.rpe);
      }

      const socket = await connectSocket(workoutId);
      const payload = clientWsMessageSchema.parse({ type: 'send_message', content: trimmed });
      socket.send(JSON.stringify(payload));
      setMessages((current) => [...current, temporaryMessage(trimmed)]);
    } catch (sendError) {
      if (sendError instanceof AuthenticationError) {
        window.location.href = '/';
        return;
      }

      setError(sendError instanceof Error ? sendError.message : 'Unable to send your message.');
    }
  }, [apiBaseUrl, connectSocket, draftRpe, ensureSummaryExists, summary?.savedAtEpochSeconds, workoutId]);

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
