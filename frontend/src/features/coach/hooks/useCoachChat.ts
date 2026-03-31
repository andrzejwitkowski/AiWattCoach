import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import {
  createWorkoutSummary,
  getWorkoutSummary,
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
  eventId: string | null;
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
  setDraftRpe: (rpe: number) => void;
  sendMessage: (content: string) => Promise<void>;
  saveSummary: () => Promise<WorkoutSummary | null>;
};

type PendingSocketState = {
  eventId: string;
  promise: Promise<WebSocket>;
};

function buildProtocol(protocol: string): 'ws:' | 'wss:' {
  return protocol === 'https:' ? 'wss:' : 'ws:';
}

export function buildWorkoutSummaryWebSocketUrl(apiBaseUrl: string, eventId: string): string {
  const path = `/api/workout-summaries/${eventId}/ws`;

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

export function useCoachChat({ apiBaseUrl, eventId }: UseCoachChatOptions): UseCoachChatResult {
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
    if (!eventId) {
      throw new Error('No workout selected.');
    }

    if (summary && summary.eventId === eventId) {
      return summary;
    }

    try {
      const created = await createWorkoutSummary(apiBaseUrl, eventId);
      setSummary(created);
      setMessages(created.messages);
      setDraftRpe(created.rpe);
      return created;
    } catch (createError) {
      if (createError instanceof AuthenticationError) {
        throw createError;
      }

      if (createError instanceof HttpError && createError.status === 409) {
        const existing = await getWorkoutSummary(apiBaseUrl, eventId);
        setSummary(existing);
        setMessages(existing.messages);
        setDraftRpe(existing.rpe);
        return existing;
      }

      throw createError;
    }
  }, [apiBaseUrl, eventId, summary]);

  const connectSocket = useCallback(async (currentEventId: string) => {
    if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
      return socketRef.current;
    }

    if (pendingSocketRef.current?.eventId === currentEventId) {
      return pendingSocketRef.current.promise;
    }

    const socketPromise = new Promise<WebSocket>((resolve, reject) => {
      const socket = new WebSocket(buildWorkoutSummaryWebSocketUrl(apiBaseUrl, currentEventId));

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

    pendingSocketRef.current = { eventId: currentEventId, promise: socketPromise };

    try {
      return await socketPromise;
    } finally {
      if (pendingSocketRef.current?.eventId === currentEventId) {
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

    if (!eventId) {
      setIsLoading(false);
      return;
    }

    let cancelled = false;

    const loadSummary = async () => {
      setIsLoading(true);

      try {
        const loadedSummary = await getWorkoutSummary(apiBaseUrl, eventId);

        if (cancelled) {
          return;
        }

        setSummary(loadedSummary);
        setMessages(loadedSummary.messages);
        setDraftRpe(loadedSummary.rpe);
        await connectSocket(eventId);
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
  }, [apiBaseUrl, closeSocket, connectSocket, eventId]);

  const saveSummary = useCallback(async () => {
    if (!eventId) {
      return null;
    }

    setIsSaving(true);
    setError(null);

    try {
      let nextSummary = summary;

      if (!nextSummary || nextSummary.eventId !== eventId) {
        nextSummary = await ensureSummaryExists();
      }

      if (draftRpe !== null && nextSummary.rpe !== draftRpe) {
        nextSummary = await updateWorkoutSummaryRpe(apiBaseUrl, eventId, draftRpe);
      }

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
  }, [apiBaseUrl, draftRpe, ensureSummaryExists, eventId, summary]);

  const sendMessage = useCallback(async (content: string) => {
    const trimmed = content.trim();

    if (!trimmed || !eventId) {
      return;
    }

    setError(null);

    try {
      await ensureSummaryExists();
      const socket = await connectSocket(eventId);
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
  }, [connectSocket, ensureSummaryExists, eventId]);

  const hasConversation = useMemo(
    () => messages.some((message) => message.role === 'coach'),
    [messages],
  );

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
    setDraftRpe,
    sendMessage,
    saveSummary,
  };
}
