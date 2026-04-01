import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useWorkoutList } from '../hooks/useWorkoutList';
import { useCoachChat } from '../hooks/useCoachChat';
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
  const workoutList = useWorkoutList({ apiBaseUrl });
  const [selectedWorkoutId, setSelectedWorkoutId] = useState<string | null>(null);
  const [showConfirmWithoutChat, setShowConfirmWithoutChat] = useState(false);
  const [isEditing, setIsEditing] = useState(true);

  const selectedItem = useMemo(
    () => workoutList.items.find((item) => item.id === selectedWorkoutId) ?? null,
    [selectedWorkoutId, workoutList.items],
  );

  const chat = useCoachChat({
    apiBaseUrl,
    workoutId: selectedItem?.id ?? null,
  });

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
    if (chat.summary?.updatedAtEpochSeconds) {
      void workoutList.refresh();
    }
  }, [chat.summary?.updatedAtEpochSeconds, workoutList.refresh]);

  async function handleSave() {
    if (chat.draftRpe === null) {
      return;
    }

    const result = await chat.saveSummary();

    if (result) {
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
                error={chat.error}
                inputDisabled={chat.isLoading || !isEditing || chat.isSaved || chat.draftRpe === null}
                onSendMessage={chat.sendMessage}
              />
              <WorkoutActionButtons
                disabled={chat.isLoading}
                isSaving={chat.isSaving}
                isEditing={isEditing}
                canSave={chat.draftRpe !== null}
                onSave={handleSaveClick}
                onEdit={() => {
                  void (async () => {
                    const result = await chat.reopenSummary();
                    if (result) {
                      setIsEditing(true);
                      await workoutList.refresh();
                    }
                  })();
                }}
              />
            </>
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
