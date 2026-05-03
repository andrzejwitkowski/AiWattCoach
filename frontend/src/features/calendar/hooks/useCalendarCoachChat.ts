import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { useApiBaseUrl } from '../../../lib/apiBaseUrl';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import { useCalendarCoachApi } from '../api/calendar';
import {
  calendarCoachClientWsMessageSchema,
  calendarCoachServerWsMessageSchema,
  type CalendarCoachConversation,
  type CalendarCoachMessage,
} from '../types';

type UseCalendarCoachChatOptions = {
  isOpen: boolean;
};

type UseCalendarCoachChatResult = {
  conversation: CalendarCoachConversation | null;
  messages: CalendarCoachMessage[];
  isLoading: boolean;
  isStartingNewConversation: boolean;
  isConnected: boolean;
  isCoachTyping: boolean;
  error: string | null;
  sendMessage: (content: string) => Promise<boolean>;
  startNewConversation: () => Promise<boolean>;
};

type PendingSocketState = {
  conversationId: string;
  socket: WebSocket;
  promise: Promise<WebSocket>;
};

function buildProtocol(protocol: string): 'ws:' | 'wss:' {
  return protocol === 'https:' ? 'wss:' : 'ws:';
}

export function buildCalendarCoachWebSocketUrl(apiBaseUrl: string, conversationId: string): string {
  const path = `/api/calendar/coach/conversations/${conversationId}/ws`;

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

function temporaryMessage(content: string): CalendarCoachMessage {
  return {
    id: `temp-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    role: 'user',
    content,
    createdAtEpochSeconds: Math.floor(Date.now() / 1000),
  };
}

function appendUniqueMessage(messages: CalendarCoachMessage[], message: CalendarCoachMessage): CalendarCoachMessage[] {
  if (messages.some((existing) => existing.id === message.id)) {
    return messages;
  }
  return [...messages, message];
}

export function useCalendarCoachChat({
  isOpen,
}: UseCalendarCoachChatOptions): UseCalendarCoachChatResult {
  const apiBaseUrl = useApiBaseUrl();
  const coachApi = useCalendarCoachApi();
  const [conversation, setConversation] = useState<CalendarCoachConversation | null>(null);
  const [messages, setMessages] = useState<CalendarCoachMessage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isStartingNewConversation, setIsStartingNewConversation] = useState(false);
  const [isConnected, setIsConnected] = useState(false);
  const [isCoachTyping, setIsCoachTyping] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const socketConversationIdRef = useRef<string | null>(null);
  const pendingSocketRef = useRef<PendingSocketState | null>(null);
  const pendingConversationRef = useRef<Promise<CalendarCoachConversation> | null>(null);
  const pendingNewConversationRef = useRef<Promise<CalendarCoachConversation> | null>(null);
  const currentConversationIdRef = useRef<string | null>(null);
  const isOpenRef = useRef(isOpen);
  const systemMessageCounterRef = useRef(0);

  useEffect(() => {
    isOpenRef.current = isOpen;
  }, [isOpen]);

  useEffect(() => {
    currentConversationIdRef.current = conversation?.conversationId ?? null;
    systemMessageCounterRef.current = 0;
  }, [conversation?.conversationId]);

  const closeSocket = useCallback(() => {
    const pendingSocket = pendingSocketRef.current;
    pendingSocketRef.current = null;
    pendingSocket?.socket.close();

    if (socketRef.current) {
      socketRef.current.close();
      socketRef.current = null;
    }

    socketConversationIdRef.current = null;
    setIsConnected(false);
    setIsCoachTyping(false);
  }, []);

  const handleAuthenticationError = useCallback((errorToCheck: unknown) => {
    if (errorToCheck instanceof AuthenticationError) {
      window.location.href = '/';
      return true;
    }

    return false;
  }, []);

  const applyConversationResponse = useCallback((nextConversation: CalendarCoachConversation, nextMessages: CalendarCoachMessage[]) => {
    currentConversationIdRef.current = nextConversation.conversationId;
    setConversation(nextConversation);
    setMessages(nextMessages);
  }, []);

  const isCurrentConversation = useCallback((conversationIdToCheck: string) => {
    return currentConversationIdRef.current === conversationIdToCheck;
  }, []);

  const connectSocket = useCallback(async (conversationId: string) => {
    if (!isOpenRef.current) {
      return null;
    }

    if (typeof WebSocket === 'undefined') {
      return null;
    }

    if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
      if (socketConversationIdRef.current === conversationId) {
        return socketRef.current;
      }

      socketRef.current.close();
      socketRef.current = null;
      socketConversationIdRef.current = null;
      setIsConnected(false);
      setIsCoachTyping(false);
    }

    if (pendingSocketRef.current && pendingSocketRef.current.conversationId !== conversationId) {
      pendingSocketRef.current.socket.close();
      pendingSocketRef.current = null;
    }

    if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
      return socketRef.current;
    }

    if (pendingSocketRef.current?.conversationId === conversationId) {
      return pendingSocketRef.current.promise;
    }

    setError(null);

    const socket = new WebSocket(buildCalendarCoachWebSocketUrl(apiBaseUrl, conversationId));
    const socketPromise = new Promise<WebSocket>((resolve, reject) => {
      socket.addEventListener('open', () => {
        if (!isOpenRef.current) {
          socket.close();
          reject(new Error('WebSocket connection no longer needed'));
          return;
        }

        if (
          pendingSocketRef.current?.socket !== socket
          || pendingSocketRef.current?.conversationId !== conversationId
        ) {
          socket.close();
          reject(new Error('WebSocket connection no longer needed'));
          return;
        }

        socketRef.current = socket;
        socketConversationIdRef.current = conversationId;
        setIsConnected(true);
        resolve(socket);
      }, { once: true });

      socket.addEventListener('message', (messageEvent) => {
        try {
          const parsed = calendarCoachServerWsMessageSchema.parse(JSON.parse(messageEvent.data as string));

          if (currentConversationIdRef.current !== conversationId) {
            return;
          }

          if (parsed.type === 'coach_typing') {
            setIsCoachTyping(true);
            return;
          }

          if (parsed.type === 'coach_message') {
            applyConversationResponse(parsed.conversation, parsed.messages);
            setIsCoachTyping(false);
            return;
          }

          if (parsed.type === 'tool_message') {
            setMessages((current) => appendUniqueMessage(current, parsed.message));
            return;
          }

          if (parsed.type === 'system_message') {
            systemMessageCounterRef.current += 1;
            setMessages((current) => [
              ...current,
              {
                id: `system-${systemMessageCounterRef.current}`,
                role: 'system',
                content: parsed.content,
                createdAtEpochSeconds: Math.floor(Date.now() / 1000),
              },
            ]);
            return;
          }

          setError(parsed.error);
          setIsCoachTyping(false);
        } catch {
          setError('Received an invalid calendar coach response.');
          setIsCoachTyping(false);
        }
      });

      socket.addEventListener('close', () => {
        if (socketRef.current === socket) {
          socketRef.current = null;
          socketConversationIdRef.current = null;
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
          socketConversationIdRef.current = null;
        }
        setError('Unable to connect to the calendar coach right now.');
        setIsConnected(false);
        setIsCoachTyping(false);
        reject(new Error('WebSocket connection failed'));
      }, { once: true });
    });

    pendingSocketRef.current = { conversationId, socket, promise: socketPromise };

    try {
      return await socketPromise;
    } finally {
      if (pendingSocketRef.current?.conversationId === conversationId) {
        pendingSocketRef.current = null;
      }
    }
  }, [apiBaseUrl, applyConversationResponse]);

  const loadConversation = useCallback(async () => {
    if (!isOpenRef.current) {
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const response = await coachApi.getCurrentCalendarCoachConversation();
      if (!isOpenRef.current) {
        return;
      }

      if (currentConversationIdRef.current && !isCurrentConversation(response.conversation.conversationId)) {
        return;
      }

      applyConversationResponse(response.conversation, response.messages);

      try {
        await connectSocket(response.conversation.conversationId);
      } catch {
        // Keep the loaded transcript even if websocket connect fails.
      }
    } catch (loadError) {
      if (!isOpenRef.current) {
        return;
      }

      if (handleAuthenticationError(loadError)) {
        return;
      }

      if (loadError instanceof HttpError && loadError.status === 404) {
        if (!currentConversationIdRef.current) {
          setConversation(null);
          setMessages([]);
        }
        return;
      }

      setError(loadError instanceof Error ? loadError.message : 'Unable to load the calendar coach conversation.');
    } finally {
      if (isOpenRef.current) {
        setIsLoading(false);
      }
    }
  }, [apiBaseUrl, applyConversationResponse, connectSocket, handleAuthenticationError, coachApi]);

  useEffect(() => {
    if (!isOpen) {
      closeSocket();
      return;
    }

    void loadConversation();

    return () => {
      closeSocket();
    };
  }, [closeSocket, isOpen, loadConversation]);

  const ensureConversation = useCallback(async () => {
    if (conversation) {
      return conversation;
    }

    if (pendingConversationRef.current) {
      return pendingConversationRef.current;
    }

    const createConversationPromise = coachApi.startNewCalendarCoachConversation()
      .then((created) => {
        if (!currentConversationIdRef.current || isCurrentConversation(created.conversation.conversationId)) {
          applyConversationResponse(created.conversation, created.messages);
        }
        return created.conversation;
      })
      .finally(() => {
        if (pendingConversationRef.current === createConversationPromise) {
          pendingConversationRef.current = null;
        }
      });

    pendingConversationRef.current = createConversationPromise;
    return createConversationPromise;
  }, [applyConversationResponse, conversation, coachApi]);

  const startNewConversation = useCallback(async () => {
    if (pendingNewConversationRef.current) {
      return pendingNewConversationRef.current.then(() => true).catch(() => false);
    }

    setIsStartingNewConversation(true);
    setError(null);
    pendingConversationRef.current = null;

    const newConversationPromise = coachApi.startNewCalendarCoachConversation()
      .then(async (created) => {
        closeSocket();
        applyConversationResponse(created.conversation, created.messages);

        try {
          await connectSocket(created.conversation.conversationId);
        } catch {
          // The new conversation exists even if websocket connect fails.
        }

        return created.conversation;
      })
      .finally(() => {
        if (pendingNewConversationRef.current === newConversationPromise) {
          pendingNewConversationRef.current = null;
        }
        setIsStartingNewConversation(false);
      });

    pendingNewConversationRef.current = newConversationPromise;

    try {
      await newConversationPromise;
      return true;
    } catch (startError) {
      pendingNewConversationRef.current = null;

      if (handleAuthenticationError(startError)) {
        return false;
      }

      setError(startError instanceof Error ? startError.message : 'Unable to start a new calendar coach conversation.');
      return false;
    }
  }, [apiBaseUrl, applyConversationResponse, closeSocket, connectSocket, handleAuthenticationError, coachApi]);

  const sendMessage = useCallback(async (content: string) => {
    const trimmed = content.trim();
    if (!trimmed) {
      return false;
    }

    setError(null);
    let attemptedConversationId: string | null = null;

    try {
      const ensuredConversation = await ensureConversation();
      const conversationId = ensuredConversation.conversationId;
      attemptedConversationId = conversationId;
      let socket: WebSocket | null = null;

      try {
        socket = await connectSocket(conversationId);
      } catch {
        socket = null;
      }

      if (socket && socket.readyState === WebSocket.OPEN) {
        if (!isCurrentConversation(conversationId)) {
          return false;
        }
        const payload = calendarCoachClientWsMessageSchema.parse({ type: 'send_message', content: trimmed });
        socket.send(JSON.stringify(payload));
        setMessages((current) => [...current, temporaryMessage(trimmed)]);
        return true;
      }

      const response = await coachApi.sendCalendarCoachMessage(conversationId, { content: trimmed });
      if (!isCurrentConversation(conversationId)) {
        return false;
      }
      applyConversationResponse(response.conversation, response.messages);
      setError(null);
      return true;
    } catch (sendError) {
      if (attemptedConversationId && !isCurrentConversation(attemptedConversationId)) {
        return false;
      }

      if (handleAuthenticationError(sendError)) {
        return false;
      }

      if (sendError instanceof HttpError && sendError.status === 404 && attemptedConversationId) {
        try {
          const reloaded = await coachApi.getCalendarCoachConversation(attemptedConversationId);
          if (isCurrentConversation(attemptedConversationId)) {
            applyConversationResponse(reloaded.conversation, reloaded.messages);
          }
        } catch {
          if (isCurrentConversation(attemptedConversationId)) {
            setConversation(null);
            setMessages([]);
          }
        }
      }

      setError(sendError instanceof Error ? sendError.message : 'Unable to send your message.');
      setIsCoachTyping(false);
      return false;
    }
  }, [apiBaseUrl, applyConversationResponse, connectSocket, ensureConversation, handleAuthenticationError, isCurrentConversation, coachApi]);

  return useMemo(() => ({
    conversation,
    messages,
    isLoading,
    isStartingNewConversation,
    isConnected,
    isCoachTyping,
    error,
    sendMessage,
    startNewConversation,
  }), [
    conversation,
    error,
    isCoachTyping,
    isConnected,
    isLoading,
    isStartingNewConversation,
    messages,
    sendMessage,
    startNewConversation,
  ]);
}
