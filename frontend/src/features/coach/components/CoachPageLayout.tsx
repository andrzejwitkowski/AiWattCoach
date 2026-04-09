import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../../settings/context/SettingsContext';
import { isAvailabilityConfigured } from '../../settings/types';
import { useWorkoutList } from '../hooks/useWorkoutList';
import { isAvailabilityRequiredChatError, useCoachChat } from '../hooks/useCoachChat';
import { ChatWindow } from './ChatWindow';
import { ConfirmWithoutChatModal } from './ConfirmWithoutChatModal';
import { EmptyWorkoutState } from './EmptyWorkoutState';
import { RpeSelector } from './RpeSelector';
import { WorkoutActionButtons } from './WorkoutActionButtons';
import { WorkoutHeader } from './WorkoutHeader';
import { WorkoutHistorySidebar } from './WorkoutHistorySidebar';

type CoachPageLayoutProps = {
  apiBaseUrl: string;
};

export function CoachPageLayout({ apiBaseUrl }: CoachPageLayoutProps) {
  const { t } = useTranslation();
  const settingsContext = useSettings();
  const workoutList = useWorkoutList({ apiBaseUrl });
  const [selectedWorkoutId, setSelectedWorkoutId] = useState<string | null>(null);
  const [showConfirmWithoutChat, setShowConfirmWithoutChat] = useState(false);
  const [isEditing, setIsEditing] = useState(true);
  const selectedWorkoutIdRef = useRef<string | null>(null);

  const selectedItem = useMemo(
    () => workoutList.items.find((item) => item.id === selectedWorkoutId) ?? null,
    [selectedWorkoutId, workoutList.items],
  );

  const availabilityConfigured = useMemo(
    () => isAvailabilityConfigured(settingsContext.settings?.availability),
    [settingsContext.settings?.availability],
  );
  const chat = useCoachChat({
    apiBaseUrl,
    workoutId: selectedItem?.id ?? null,
  });
  const hasSettingsLoadError = Boolean(settingsContext.error);
  const requiresAvailability = (!settingsContext.isLoading && !hasSettingsLoadError && !availabilityConfigured)
    || isAvailabilityRequiredChatError(chat.error);

  useEffect(() => {
    selectedWorkoutIdRef.current = selectedWorkoutId;
  }, [selectedWorkoutId]);

  useEffect(() => {
    if (workoutList.items.length === 0) {
      setSelectedWorkoutId(null);
      return;
    }

    if (!selectedWorkoutId || !workoutList.items.some((item) => item.id === selectedWorkoutId)) {
      setSelectedWorkoutId(workoutList.items[0].id);
    }
  }, [selectedWorkoutId, workoutList.items]);

  useEffect(() => {
    setShowConfirmWithoutChat(false);
    setIsEditing(!(selectedItem?.summary?.savedAtEpochSeconds ?? null));
  }, [selectedItem?.summary?.savedAtEpochSeconds, selectedWorkoutId]);

  useEffect(() => {
    if (chat.summary) {
      workoutList.replaceSummary(chat.summary);
    }
  }, [chat.summary, workoutList.replaceSummary]);

  const isCurrentSelection = useCallback((workoutId: string) => {
    return selectedWorkoutIdRef.current === workoutId;
  }, []);

  async function handleSave() {
    if (chat.draftRpe === null || !selectedItem) {
      return;
    }

    const workoutId = selectedItem.id;

    const result = await chat.saveSummary();

    if (result && isCurrentSelection(workoutId)) {
      workoutList.replaceSummary(result);
      setIsEditing(false);
      setShowConfirmWithoutChat(false);
      await workoutList.refresh();
    }
  }

  function handleSaveClick() {
    if (!selectedItem) {
      return;
    }

    if (chat.draftRpe === null) {
      return;
    }

    if (!chat.hasConversation) {
      setShowConfirmWithoutChat(true);
      return;
    }

    void handleSave();
  }

  return (
    <section className="space-y-6">
      <div className="grid gap-6 xl:grid-cols-[22rem_minmax(0,1fr)]">
        <WorkoutHistorySidebar
          items={workoutList.items}
          selectedWorkoutId={selectedWorkoutId}
          state={workoutList.state}
          error={workoutList.error}
          weekLabel={workoutList.weekLabel}
          canGoToNewerWeek={workoutList.canGoToNewerWeek}
          onOlderWeek={workoutList.goToOlderWeek}
          onNewerWeek={workoutList.goToNewerWeek}
          onSelectWorkout={setSelectedWorkoutId}
        />

        <div className="space-y-6">
          {selectedItem ? (
            <>
              <WorkoutHeader item={selectedItem} hasConversation={chat.hasConversation} />
              <RpeSelector
                value={chat.draftRpe}
                disabled={!isEditing || chat.isLoading}
                onChange={chat.setDraftRpe}
              />
              <ChatWindow
                messages={chat.messages}
                isCoachTyping={chat.isCoachTyping}
                isConnected={chat.isConnected}
                hasSelectedWorkout
                isSaved={chat.isSaved}
                requiresRpe={chat.draftRpe === null}
                requiresAvailability={requiresAvailability}
                availabilityMessage={
                  requiresAvailability
                    ? t('coach.chatAvailabilityRequiredBanner')
                    : null
                }
                error={chat.error ?? (hasSettingsLoadError ? settingsContext.error : null)}
                inputDisabled={
                  chat.isLoading
                  || !isEditing
                  || chat.isSaved
                  || chat.draftRpe === null
                  || settingsContext.isLoading
                  || hasSettingsLoadError
                  || requiresAvailability
                }
                onSendMessage={chat.sendMessage}
              />
              <WorkoutActionButtons
                disabled={chat.isLoading}
                isSaving={chat.isSaving}
                isEditing={isEditing}
                canSave={chat.draftRpe !== null}
                onSave={handleSaveClick}
                onEdit={() => {
                  if (!selectedItem) {
                    return;
                  }

                  const workoutId = selectedItem.id;
                  void (async () => {
                    const result = await chat.reopenSummary();
                    if (result && isCurrentSelection(workoutId)) {
                      workoutList.replaceSummary(result);
                      setIsEditing(true);
                      await workoutList.refresh();
                    }
                  })();
                }}
              />
            </>
          ) : workoutList.state === 'credentials-required' ? (
            <div className="glass-panel rounded-2xl border border-amber-300/20 bg-amber-300/10 p-10 text-center text-amber-100">
              {t('calendar.connectionRequired')}
            </div>
          ) : workoutList.state === 'error' ? (
            <div className="glass-panel rounded-2xl border border-red-400/25 bg-red-500/10 p-10 text-center text-red-200">
              {workoutList.error ?? t('coach.loadingError')}
            </div>
          ) : workoutList.state === 'ready' ? (
            <EmptyWorkoutState />
          ) : (
            <div className="glass-panel rounded-2xl border border-white/10 p-10 text-center text-slate-400">
              {t('coach.loadingWorkouts')}
            </div>
          )}
        </div>
      </div>
      <ConfirmWithoutChatModal
        isOpen={showConfirmWithoutChat}
        isSaving={chat.isSaving}
        onCancel={() => {
          setShowConfirmWithoutChat(false);
        }}
        onConfirm={() => {
          void handleSave();
        }}
      />
    </section>
  );
}
