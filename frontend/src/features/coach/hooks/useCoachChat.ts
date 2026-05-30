import { useCallback, useEffect, useRef, useState } from 'react';

import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import {
  createWorkoutSummary,
  getWorkoutSummary,
  reopenWorkoutSummary,
  saveWorkoutSummary,
  sendWorkoutSummaryMessage,
  updateWorkoutSummaryRpe,
  type WorkoutSummaryDateRange,
} from '../api/workoutSummary';
import {
  clientWsMessageSchema,
  type CoachChatProgressState,
  type ConversationMessage,
  type SaveWorkoutSummaryResponse,
  type WorkoutSummary,
} from '../types';
import {
  buildWorkoutSummaryWebSocketUrl,
  useCoachChatSocket,
} from './useCoachChatSocket';
import { useCoachChatSummaryState } from './useCoachChatSummaryState';

type UseCoachChatOptions = {
  apiBaseUrl: string;
  workoutId: string | null;
  aliasRange?: WorkoutSummaryDateRange | null;
};

type UseCoachChatResult = {
  summary: WorkoutSummary | null;
  messages: ConversationMessage[];
  draftRpe: number | null;
  isLoading: boolean;
  isSaving: boolean;
  isConnected: boolean;
  isCoachTyping: boolean;
  progressState: CoachChatProgressState;
  error: string | null;
  hasConversation: boolean;
  isSaved: boolean;
  setDraftRpe: (rpe: number) => void;
  sendMessage: (content: string) => Promise<boolean>;
  saveSummary: () => Promise<SaveWorkoutSummaryResponse | null>;
  reopenSummary: () => Promise<WorkoutSummary | null>;
};

export const availabilityRequiredChatError = 'availability must be configured before chatting with coach';

export function isAvailabilityRequiredChatError(error: string | null | undefined): boolean {
  return /availability\s+must\s+be\s+configured\s+before\s+chatting\s+with\s+coach/i.test(error ?? '');
}

class StaleWorkoutSelectionError extends Error {
  constructor() {
    super('Workout selection changed before the request completed.');
  }
}

export { buildWorkoutSummaryWebSocketUrl };

export function useCoachChat({
  apiBaseUrl,
  workoutId,
  aliasRange = null,
}: UseCoachChatOptions): UseCoachChatResult {
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [progressState, setProgressState] = useState<CoachChatProgressState>('idle');
  const [error, setError] = useState<string | null>(null);
  const aliasRangeOldest = aliasRange?.oldest ?? null;
  const aliasRangeNewest = aliasRange?.newest ?? null;
  const currentWorkoutIdRef = useRef<string | null>(workoutId);
  const savingRequestIdRef = useRef(0);

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

  const clearReplyProgress = useCallback(() => {
    setProgressState((current) => (current === 'awaiting-reply' ? 'idle' : current));
  }, []);

  const isCurrentWorkout = useCallback((candidateWorkoutId: string) => {
    return currentWorkoutIdRef.current === candidateWorkoutId;
  }, []);

  const {
    appendSystemMessage,
    appendTemporaryUserMessage,
    appendToolMessage,
    appendWorkflowMessages,
    applyIncomingSummary,
    clearSummaryState,
    draftRpe,
    hasConversation,
    isSaved,
    messages,
    resetTransientState,
    setDraftRpeOverride,
    summary,
  } = useCoachChatSummaryState({
    workoutId,
    assertCurrentWorkout,
  });

  const handleSetDraftRpe = useCallback((rpe: number) => {
    setDraftRpeOverride(rpe);
    setError(null);
  }, [setDraftRpeOverride]);

  const handleCoachMessage = useCallback((nextSummary: WorkoutSummary, currentWorkoutId: string) => {
    applyIncomingSummary(nextSummary, currentWorkoutId);
  }, [applyIncomingSummary]);

  const { closeSocket, connectSocket, isConnected, isCoachTyping } = useCoachChatSocket({
    apiBaseUrl,
    clearReplyProgress,
    isCurrentWorkout,
    onCoachMessage: handleCoachMessage,
    onToolMessage: appendToolMessage,
    onSystemMessage: appendSystemMessage,
    onWorkflowMessages: appendWorkflowMessages,
    onError: setError,
  });

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
      applyIncomingSummary(created, requestedWorkoutId);
      return created;
    } catch (createError) {
      if (createError instanceof AuthenticationError) {
        throw createError;
      }

      if (createError instanceof HttpError && createError.status === 409) {
        const existing = await getWorkoutSummary(apiBaseUrl, requestedWorkoutId);
        applyIncomingSummary(existing, requestedWorkoutId);
        return existing;
      }

      throw createError;
    }
  }, [apiBaseUrl, applyIncomingSummary, summary, workoutId]);

  useEffect(() => {
    closeSocket();
    resetTransientState();
    setError(null);
    setProgressState('idle');

    if (!workoutId) {
      setIsLoading(false);
      return;
    }

    let cancelled = false;

    const loadSummary = async () => {
      setIsLoading(true);

      try {
        const loadedSummary = await getWorkoutSummary(
          apiBaseUrl,
          workoutId,
          aliasRangeOldest && aliasRangeNewest
            ? { oldest: aliasRangeOldest, newest: aliasRangeNewest }
            : undefined,
        );

        if (cancelled) {
          return;
        }

        applyIncomingSummary(loadedSummary, workoutId);
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
  }, [
    aliasRangeNewest,
    aliasRangeOldest,
    apiBaseUrl,
    applyIncomingSummary,
    clearSummaryState,
    closeSocket,
    connectSocket,
    resetTransientState,
    workoutId,
  ]);

  const saveSummary = useCallback(async () => {
    if (!workoutId) {
      return null;
    }

    const requestedWorkoutId = workoutId;
    const requestId = savingRequestIdRef.current + 1;
    savingRequestIdRef.current = requestId;

    setIsSaving(true);
    setProgressState('saving-summary');
    setError(null);

    try {
      let nextSummary = summary;

      if (!nextSummary || nextSummary.workoutId !== requestedWorkoutId) {
        nextSummary = await ensureSummaryExists();
      }

      assertCurrentWorkout(requestedWorkoutId);

      if (draftRpe !== null && nextSummary.rpe !== draftRpe) {
        nextSummary = await updateWorkoutSummaryRpe(apiBaseUrl, requestedWorkoutId, draftRpe);
        applyIncomingSummary(nextSummary, requestedWorkoutId);
      }

      const saveResult = await saveWorkoutSummary(apiBaseUrl, requestedWorkoutId);
      applyIncomingSummary(saveResult.summary, requestedWorkoutId);

      if (currentWorkoutIdRef.current === requestedWorkoutId) {
        appendWorkflowMessages(saveResult.workflow.messages);
      }

      return saveResult;
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
        setProgressState('idle');
      }
    }
  }, [
    apiBaseUrl,
    appendWorkflowMessages,
    applyIncomingSummary,
    assertCurrentWorkout,
    draftRpe,
    ensureSummaryExists,
    summary,
    workoutId,
  ]);

  const reopenSummary = useCallback(async () => {
    if (!workoutId) {
      return null;
    }

    const requestedWorkoutId = workoutId;
    const requestId = savingRequestIdRef.current + 1;
    savingRequestIdRef.current = requestId;

    setIsSaving(true);
    setProgressState('saving-summary');
    setError(null);

    try {
      const reopenedSummary = await reopenWorkoutSummary(apiBaseUrl, requestedWorkoutId);
      applyIncomingSummary(reopenedSummary, requestedWorkoutId);
      return reopenedSummary;
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
        setProgressState('idle');
      }
    }
  }, [apiBaseUrl, applyIncomingSummary, workoutId]);

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
        applyIncomingSummary(nextSummary, requestedWorkoutId);
      }

      let socket: WebSocket | null = null;
      try {
        socket = await connectSocket(requestedWorkoutId);
      } catch {
        setError(null);
        socket = null;
      }

      if (socket && socket.readyState === WebSocket.OPEN) {
        assertCurrentWorkout(requestedWorkoutId);
        const payload = clientWsMessageSchema.parse({ type: 'send_message', content: trimmed });
        setProgressState('awaiting-reply');
        socket.send(JSON.stringify(payload));

        if (currentWorkoutIdRef.current === requestedWorkoutId) {
          appendTemporaryUserMessage(trimmed);
        }

        return true;
      }

      const response = await sendWorkoutSummaryMessage(apiBaseUrl, requestedWorkoutId, { content: trimmed });
      assertCurrentWorkout(requestedWorkoutId);
      applyIncomingSummary(response.summary, requestedWorkoutId);
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
      clearReplyProgress();
      return false;
    }
  }, [
    apiBaseUrl,
    appendTemporaryUserMessage,
    applyIncomingSummary,
    assertCurrentWorkout,
    clearReplyProgress,
    connectSocket,
    draftRpe,
    ensureSummaryExists,
    summary?.savedAtEpochSeconds,
    workoutId,
  ]);

  return {
    summary,
    messages,
    draftRpe,
    isLoading,
    isSaving,
    isConnected,
    isCoachTyping,
    progressState,
    error,
    hasConversation,
    isSaved,
    setDraftRpe: handleSetDraftRpe,
    sendMessage,
    saveSummary,
    reopenSummary,
  };
}
