import { useCallback, useRef, useState } from 'react';

import { serverWsMessageSchema, type ConversationMessage, type WorkoutSummary } from '../types';

type PendingSocketState = {
  workoutId: string;
  socket: WebSocket;
  promise: Promise<WebSocket>;
  reject: (error: Error) => void;
};

type UseCoachChatSocketOptions = {
  apiBaseUrl: string;
  clearReplyProgress: () => void;
  isCurrentWorkout: (workoutId: string) => boolean;
  onCoachMessage: (summary: WorkoutSummary, workoutId: string) => void;
  onToolMessage: (message: ConversationMessage) => void;
  onSystemMessage: (content: string) => void;
  onWorkflowMessages: (messages: string[]) => void;
  onError: (message: string | null) => void;
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
    const safeBase = apiBaseUrl.replace(/\/+$/, '');
    return `${buildProtocol(window.location.protocol)}//${window.location.host}${safeBase}${path}`;
  }

  const url = new URL(apiBaseUrl);
  const normalizedBasePath = url.pathname.endsWith('/')
    ? url.pathname.slice(0, -1)
    : url.pathname;
  url.pathname = `${normalizedBasePath}${path}`;
  url.protocol = buildProtocol(url.protocol);
  return url.toString();
}

export function useCoachChatSocket({
  apiBaseUrl,
  clearReplyProgress,
  isCurrentWorkout,
  onCoachMessage,
  onToolMessage,
  onSystemMessage,
  onWorkflowMessages,
  onError,
}: UseCoachChatSocketOptions) {
  const [isConnected, setIsConnected] = useState(false);
  const [isCoachTyping, setIsCoachTyping] = useState(false);
  const socketRef = useRef<WebSocket | null>(null);
  const socketWorkoutIdRef = useRef<string | null>(null);
  const pendingSocketRef = useRef<PendingSocketState | null>(null);

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
    clearReplyProgress();
  }, [clearReplyProgress]);

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

    onError(null);

    const socket = new WebSocket(buildWorkoutSummaryWebSocketUrl(apiBaseUrl, currentWorkoutId));

    let pendingReject: ((error: Error) => void) | null = null;

    const socketPromise = new Promise<WebSocket>((resolve, reject) => {
      pendingReject = reject;
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

          if (!isCurrentWorkout(currentWorkoutId)) {
            return;
          }

          if (parsed.type === 'coach_typing') {
            setIsCoachTyping(true);
            return;
          }

          if (parsed.type === 'coach_message') {
            onCoachMessage(parsed.summary, currentWorkoutId);
            setIsCoachTyping(false);
            clearReplyProgress();
            return;
          }

          if (parsed.type === 'tool_message') {
            onToolMessage(parsed.message);
            return;
          }

          if (parsed.type === 'system_message') {
            onSystemMessage(parsed.content);
            setIsCoachTyping(false);
            return;
          }

          if (parsed.type === 'save_workflow_complete') {
            onWorkflowMessages(parsed.workflow.messages);
            setIsCoachTyping(false);
            clearReplyProgress();
            return;
          }

          onError(parsed.error);
          setIsCoachTyping(false);
          clearReplyProgress();
        } catch {
          onError('Received an invalid coach response.');
          setIsCoachTyping(false);
          clearReplyProgress();
        }
      });

      socket.addEventListener('close', () => {
        if (socketRef.current === socket) {
          socketRef.current = null;
          socketWorkoutIdRef.current = null;
          setIsConnected(false);
        }
        if (pendingSocketRef.current?.socket === socket) {
          pendingSocketRef.current.reject(new Error('WebSocket closed before open'));
          pendingSocketRef.current = null;
        }
        setIsCoachTyping(false);
        clearReplyProgress();
      });

      socket.addEventListener('error', () => {
        if (pendingSocketRef.current?.socket === socket) {
          pendingSocketRef.current = null;
        }
        if (socketRef.current === socket) {
          socketRef.current = null;
          socketWorkoutIdRef.current = null;
        }
        onError('Unable to connect to the coach chat right now.');
        setIsConnected(false);
        setIsCoachTyping(false);
        clearReplyProgress();
        reject(new Error('WebSocket connection failed'));
      }, { once: true });
    });

    pendingSocketRef.current = { workoutId: currentWorkoutId, socket, promise: socketPromise, reject: pendingReject! };

    try {
      return await socketPromise;
    } finally {
      if (pendingSocketRef.current?.workoutId === currentWorkoutId) {
        pendingSocketRef.current = null;
      }
    }
  }, [apiBaseUrl, clearReplyProgress, isCurrentWorkout, onCoachMessage, onError, onSystemMessage, onToolMessage, onWorkflowMessages]);

  return {
    closeSocket,
    connectSocket,
    isConnected,
    isCoachTyping,
  };
}
