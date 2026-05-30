import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { useCoachSessionCache } from '../context/CoachSessionCache';
import type { ConversationMessage, WorkoutSummary } from '../types';

export type SummaryApplyMode = 'force' | 'newer-only';

function temporaryMessage(content: string): ConversationMessage {
  return {
    id: `temp-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    role: 'user',
    content,
    createdAtEpochSeconds: Math.floor(Date.now() / 1000),
  };
}

function localSystemMessages(contents: string[], startId: number): ConversationMessage[] {
  const createdAtEpochSeconds = Math.floor(Date.now() / 1000);
  return contents.map((content, index) => ({
    id: `system-${startId + index}`,
    role: 'system',
    content,
    createdAtEpochSeconds,
  }));
}

function appendUniqueMessage(messages: ConversationMessage[], message: ConversationMessage): ConversationMessage[] {
  if (messages.some((existing) => existing.id === message.id)) {
    return messages;
  }
  return [...messages, message];
}

function incomingSummaryIsNewer(current: WorkoutSummary | null, incoming: WorkoutSummary): boolean {
  if (!current || current.workoutId !== incoming.workoutId) {
    return true;
  }

  if (incoming.updatedAtEpochSeconds !== current.updatedAtEpochSeconds) {
    return incoming.updatedAtEpochSeconds > current.updatedAtEpochSeconds;
  }

  return incoming.messages.length > current.messages.length;
}

type UseCoachChatSummaryStateOptions = {
  workoutId: string | null;
  assertCurrentWorkout: (expectedWorkoutId: string) => void;
};

type UseCoachChatSummaryStateResult = {
  summary: WorkoutSummary | null;
  messages: ConversationMessage[];
  draftRpe: number | null;
  hasConversation: boolean;
  isSaved: boolean;
  setDraftRpeOverride: (rpe: number) => void;
  resetTransientState: () => void;
  appendToolMessage: (message: ConversationMessage) => void;
  appendSystemMessage: (content: string) => void;
  appendWorkflowMessages: (contents: string[]) => void;
  appendTemporaryUserMessage: (content: string) => void;
  applyIncomingSummary: (
    nextSummary: WorkoutSummary,
    expectedWorkoutId: string,
    mode?: SummaryApplyMode,
  ) => boolean;
  clearSummaryState: (expectedWorkoutId: string) => void;
};

export function useCoachChatSummaryState({
  workoutId,
  assertCurrentWorkout,
}: UseCoachChatSummaryStateOptions): UseCoachChatSummaryStateResult {
  const { clearSummaries, getSummary, revision, upsertFullSummary } = useCoachSessionCache();
  const [draftRpeOverride, setDraftRpeOverride] = useState<number | null>(null);
  const [localOnlyMessages, setLocalOnlyMessages] = useState<ConversationMessage[]>([]);
  const localSystemMessageIdRef = useRef(0);

  const summary = useMemo(
    () => (workoutId ? getSummary(workoutId) ?? null : null),
    [getSummary, revision, workoutId],
  );
  const messages = useMemo(() => {
    const persisted = summary?.messages ?? [];
    return localOnlyMessages.length === 0 ? persisted : [...persisted, ...localOnlyMessages];
  }, [localOnlyMessages, summary?.messages]);
  const draftRpe = draftRpeOverride ?? summary?.rpe ?? null;

  const resetTransientState = useCallback(() => {
    setDraftRpeOverride(null);
    setLocalOnlyMessages([]);
  }, []);

  useEffect(() => {
    resetTransientState();
  }, [resetTransientState, workoutId]);

  const applyIncomingSummary = useCallback((
    nextSummary: WorkoutSummary,
    expectedWorkoutId: string,
    mode: SummaryApplyMode = 'force',
  ) => {
    assertCurrentWorkout(expectedWorkoutId);

    const current = getSummary(expectedWorkoutId) ?? null;
    if (mode === 'newer-only' && !incomingSummaryIsNewer(current, nextSummary)) {
      return false;
    }

    upsertFullSummary(nextSummary);
    resetTransientState();
    return true;
  }, [assertCurrentWorkout, getSummary, resetTransientState, upsertFullSummary]);

  const clearSummaryState = useCallback((expectedWorkoutId: string) => {
    assertCurrentWorkout(expectedWorkoutId);
    resetTransientState();
    clearSummaries([expectedWorkoutId]);
  }, [assertCurrentWorkout, clearSummaries, resetTransientState]);

  const appendToolMessage = useCallback((message: ConversationMessage) => {
    setLocalOnlyMessages((current) => appendUniqueMessage(current, message));
  }, []);

  const appendSystemMessage = useCallback((content: string) => {
    localSystemMessageIdRef.current += 1;
    setLocalOnlyMessages((current) => [
      ...current,
      {
        id: `system-${localSystemMessageIdRef.current}`,
        role: 'system',
        content,
        createdAtEpochSeconds: Math.floor(Date.now() / 1000),
      },
    ]);
  }, []);

  const appendWorkflowMessages = useCallback((contents: string[]) => {
    if (contents.length === 0) {
      return;
    }

    const startId = localSystemMessageIdRef.current + 1;
    localSystemMessageIdRef.current += contents.length;
    setLocalOnlyMessages((current) => [...current, ...localSystemMessages(contents, startId)]);
  }, []);

  const appendTemporaryUserMessage = useCallback((content: string) => {
    setLocalOnlyMessages((current) => [...current, temporaryMessage(content)]);
  }, []);

  const hasConversation = useMemo(
    () => messages.some((message) => message.role === 'coach'),
    [messages],
  );

  const isSaved = summary?.savedAtEpochSeconds !== null && summary?.savedAtEpochSeconds !== undefined;

  return {
    summary,
    messages,
    draftRpe,
    hasConversation,
    isSaved,
    setDraftRpeOverride,
    resetTransientState,
    appendToolMessage,
    appendSystemMessage,
    appendWorkflowMessages,
    appendTemporaryUserMessage,
    applyIncomingSummary,
    clearSummaryState,
  };
}
