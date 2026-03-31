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
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);
  const [showConfirmWithoutChat, setShowConfirmWithoutChat] = useState(false);
  const [isEditing, setIsEditing] = useState(true);

  const selectedItem = useMemo(
    () => workoutList.items.find((item) => String(item.event.id) === selectedEventId) ?? null,
    [selectedEventId, workoutList.items],
  );

  const chat = useCoachChat({
    apiBaseUrl,
    eventId: selectedItem ? String(selectedItem.event.id) : null,
  });

  useEffect(() => {
    if (workoutList.items.length === 0) {
      setSelectedEventId(null);
      return;
    }

    if (!selectedEventId || !workoutList.items.some((item) => String(item.event.id) === selectedEventId)) {
      setSelectedEventId(String(workoutList.items[0].event.id));
    }
  }, [selectedEventId, workoutList.items]);

  useEffect(() => {
    setShowConfirmWithoutChat(false);
    setIsEditing(true);
  }, [selectedEventId]);

  useEffect(() => {
    if (chat.summary?.updatedAtEpochSeconds) {
      void workoutList.refresh();
    }
  }, [chat.summary?.updatedAtEpochSeconds, workoutList.refresh]);

  async function handleSave() {
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
          selectedEventId={selectedEventId}
          state={workoutList.state}
          error={workoutList.error}
          weekLabel={workoutList.weekLabel}
          canGoToNewerWeek={workoutList.canGoToNewerWeek}
          onOlderWeek={workoutList.goToOlderWeek}
          onNewerWeek={workoutList.goToNewerWeek}
          onSelectWorkout={setSelectedEventId}
        />

        <div className="space-y-6">
          {selectedItem ? (
            <>
              <WorkoutHeader event={selectedItem.event} hasConversation={chat.hasConversation} />
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
                error={chat.error}
                inputDisabled={chat.isLoading}
                onSendMessage={chat.sendMessage}
              />
              <WorkoutActionButtons
                disabled={chat.isLoading}
                isSaving={chat.isSaving}
                isEditing={isEditing}
                onSave={handleSaveClick}
                onEdit={() => {
                  setIsEditing(true);
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
