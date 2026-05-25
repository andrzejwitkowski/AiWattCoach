import {X} from 'lucide-react';
import {useEffect, useState} from 'react';
import {useTranslation} from 'react-i18next';

import {useApiBaseUrl} from '../../../lib/apiBaseUrl';
import {downloadFit, loadActivity, loadEvent} from '../../intervals/api/intervals';
import {AuthenticationError} from '../../../lib/httpClient';
import type {IntervalEvent} from '../../intervals/types';
import {useCompletedWorkoutSummary} from '../hooks/useCompletedWorkoutSummary';
import type {WorkoutDetailSelection} from '../workoutDetails';
import {CompletedWorkoutDetailModal} from './CompletedWorkoutDetailModal';
import {PlannedWorkoutDetailModal} from './PlannedWorkoutDetailModal';

type WorkoutDetailModalProps = {
    selection: WorkoutDetailSelection | null;
    onClose: () => void;
    onEventSynced?: (event: IntervalEvent) => void;
};

type ModalState = {
    event: WorkoutDetailSelection['event'];
    activity: WorkoutDetailSelection['activity'];
    loading: boolean;
};

export function WorkoutDetailModal({selection, onClose, onEventSynced}: WorkoutDetailModalProps) {
    const apiBaseUrl = useApiBaseUrl();
    const {t} = useTranslation();
    const [state, setState] = useState<ModalState>({event: null, activity: null, loading: false});
    const [downloadingFit, setDownloadingFit] = useState(false);
    const [syncingToIntervals, setSyncingToIntervals] = useState(false);
    const [syncingToWahoo, setSyncingToWahoo] = useState(false);
    const [syncError, setSyncError] = useState<string | null>(null);

    useEffect(() => {
        if (!selection) {
            setState({event: null, activity: null, loading: false});
            setSyncError(null);
            return;
        }

        let cancelled = false;
        setSyncError(null);
        setState({event: null, activity: null, loading: true});

        void Promise.allSettled([
            selection.event && (selection.event.plannedSource !== 'predicted' || selection.event.linkedIntervalsEventId)
                ? loadEvent(apiBaseUrl, selection.event.linkedIntervalsEventId ?? selection.event.id)
                : Promise.resolve(null),
            selection.activity ? loadActivity(apiBaseUrl, selection.activity.id) : Promise.resolve(null),
        ]).then(([eventResult, activityResult]) => {
            if (cancelled) {
                return;
            }

            const authError = [eventResult, activityResult]
                .filter((result): result is PromiseRejectedResult => result.status === 'rejected')
                .find((result) => result.reason instanceof AuthenticationError);
            if (authError) {
                window.location.href = '/';
                return;
            }

            setState({
                event: eventResult.status === 'fulfilled'
                    ? mergeSelectedEvent(selection.event, eventResult.value)
                    : selection.event,
                activity: activityResult.status === 'fulfilled' ? activityResult.value : selection.activity,
                loading: false,
            });
        });

        return () => {
            cancelled = true;
        };
    }, [apiBaseUrl, selection]);

    const event = selection ? state.event : null;
    const activity = selection ? state.activity : null;
    const actualWorkout = event?.actualWorkout ?? null;
    const isCompletedActivityOnly = Boolean(!event && activity);
    const isPlannedVsActual = Boolean(event && actualWorkout);
    const isCompleted = Boolean(actualWorkout || isCompletedActivityOnly);
    const summaryTargetActivityId = selection
        ? event?.actualWorkout?.activityId ?? activity?.id ?? null
        : null;
    const {
        summary: workoutSummary,
        isLoading: isSummaryLoading,
        summaryError,
    } = useCompletedWorkoutSummary({ activityId: summaryTargetActivityId });

    if (!selection) {
        return null;
    }
    const title = isCompletedActivityOnly
        ? activity?.name ?? activity?.activityType ?? t('calendar.workout')
        : isPlannedVsActual
            ? actualWorkout?.activityName ?? event?.name ?? t('calendar.workout')
            : event?.name
                ?? selection.event?.name
                ?? t('calendar.workout');
    const hasIntervalsEventId = Boolean(event && event.id > 0 && (event.plannedSource !== 'predicted' || event.linkedIntervalsEventId));
    const showFitDownload = Boolean(event && !isCompleted && hasIntervalsEventId);

    const handleDownloadFit = async () => {
        if (!event || downloadingFit) {
            return;
        }

        try {
            setDownloadingFit(true);
            const downloadEventId = event.linkedIntervalsEventId ?? event.id;
            const fitBytes = await downloadFit(apiBaseUrl, downloadEventId);
            const fitFileBytes = Uint8Array.from(fitBytes);
            const blob = new Blob([fitFileBytes], {type: 'application/octet-stream'});
            const objectUrl = URL.createObjectURL(blob);
            const link = document.createElement('a');
            link.href = objectUrl;
            link.download = `event-${downloadEventId}.fit`;
            link.click();
            window.setTimeout(() => URL.revokeObjectURL(objectUrl), 0);
        } catch (error: unknown) {
            if (error instanceof AuthenticationError) {
                window.location.href = '/';
            }
        } finally {
            setDownloadingFit(false);
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/65 px-3 py-3 backdrop-blur-sm md:px-4 md:py-6">
            <div
                className="w-full max-w-5xl overflow-hidden rounded-[1.2rem] border border-white/8 bg-[#111417] shadow-[0_24px_80px_rgba(0,0,0,0.5)] md:rounded-[1.5rem]">
                <div className="flex items-start justify-between gap-3 border-b border-white/6 px-4 py-4 md:items-center md:px-8">
                    <div>
                        <p className="text-[10px] font-black uppercase tracking-[0.28em] text-slate-500">
                            {isCompleted ? t('calendar.completedWorkout') : t('calendar.plannedWorkout')}
                        </p>
                        <h2 className="mt-2 text-xl font-black uppercase tracking-tight text-[#f9f9fd] md:text-3xl">{title}</h2>
                    </div>
                    <div className="flex shrink-0 items-center gap-2 md:gap-3">
                        {showFitDownload ? (
                            <button
                                type="button"
                                onClick={() => void handleDownloadFit()}
                                disabled={downloadingFit}
                                className="rounded-full border border-white/10 bg-white/5 px-3 py-2 text-[10px] font-bold uppercase tracking-[0.18em] text-slate-200 transition hover:bg-white/10 hover:text-white disabled:cursor-wait disabled:opacity-60 md:px-4 md:text-xs md:tracking-[0.2em]"
                            >
                                {downloadingFit ? t('calendar.downloadingFit') : t('calendar.downloadFit')}
                            </button>
                        ) : null}
                        <button
                            type="button"
                            onClick={onClose}
                            aria-label={t('calendar.closeWorkoutDetails')}
                            className="rounded-full border border-white/10 bg-white/5 p-2 text-slate-300 transition hover:bg-white/10 hover:text-white"
                        >
                            <X size={18}/>
                        </button>
                    </div>
                </div>

                <div className="max-h-[calc(100dvh-7rem)] overflow-y-auto px-4 py-4 md:max-h-[80vh] md:px-8 md:py-6">
                    {state.loading ? (
                        <p className="text-sm text-slate-400">{t('calendar.loadingWorkoutDetails')}</p>
                    ) : isCompleted ? (
                        <CompletedWorkoutDetailModal
                            event={event}
                            activity={activity}
                            workoutSummary={workoutSummary}
                            isSummaryLoading={isSummaryLoading}
                            summaryError={summaryError}
                        />
                    ) : event ? (
                        <PlannedWorkoutDetailModal
                            event={event}
                            syncingToIntervals={syncingToIntervals}
                            syncingToWahoo={syncingToWahoo}
                            onSyncingToIntervalsChange={setSyncingToIntervals}
                            onSyncingToWahooChange={setSyncingToWahoo}
                            onSyncError={setSyncError}
                            onEventSynced={(syncedEvent) => {
                                setState((current) => {
                                    if (!current.event || !isSameSelectedEvent(current.event, syncedEvent)) {
                                        return current;
                                    }

                                    return {...current, event: syncedEvent};
                                });
                                onEventSynced?.(syncedEvent);
                            }}
                        />
                    ) : (
                        <p className="text-sm text-slate-400">{t('calendar.workoutDetailsUnavailable')}</p>
                    )}
                    {syncError ? (
                        <div className="mt-4 rounded-2xl border border-amber-300/20 bg-amber-300/10 p-4 text-sm text-amber-100">
                            {syncError}
                        </div>
                    ) : null}
                </div>
            </div>
        </div>
    );
}

function mergeSelectedEvent(
    selectedEvent: WorkoutDetailSelection['event'],
    loadedEvent: IntervalEvent | null,
): WorkoutDetailSelection['event'] {
    if (!selectedEvent) {
        return loadedEvent;
    }

    if (!loadedEvent) {
        return selectedEvent;
    }

    return {
        ...loadedEvent,
        eventDefinition: hasMeaningfulEventDefinition(loadedEvent.eventDefinition)
            ? loadedEvent.eventDefinition
            : mergeEventDefinitions(selectedEvent.eventDefinition, loadedEvent.eventDefinition),
        calendarEntryId: selectedEvent.calendarEntryId,
        plannedSource: selectedEvent.plannedSource,
        syncStatus: selectedEvent.syncStatus,
        linkedIntervalsEventId: selectedEvent.linkedIntervalsEventId ?? loadedEvent.linkedIntervalsEventId,
        projectedWorkout: selectedEvent.projectedWorkout,
    };
}

function hasMeaningfulEventDefinition(eventDefinition: IntervalEvent['eventDefinition']): boolean {
    return (
        eventDefinition.intervals.length > 0
        || eventDefinition.segments.length > 0
    );
}

function isSameSelectedEvent(currentEvent: IntervalEvent, nextEvent: IntervalEvent): boolean {
    const currentProjectedWorkoutId = currentEvent.projectedWorkout?.projectedWorkoutId;
    const nextProjectedWorkoutId = nextEvent.projectedWorkout?.projectedWorkoutId;
    if (currentProjectedWorkoutId && nextProjectedWorkoutId) {
        return currentProjectedWorkoutId === nextProjectedWorkoutId;
    }

    return currentEvent.calendarEntryId === nextEvent.calendarEntryId;
}

function mergeEventDefinitions(
    selectedDefinition: IntervalEvent['eventDefinition'],
    loadedDefinition: IntervalEvent['eventDefinition'],
): IntervalEvent['eventDefinition'] {
    return {
        ...loadedDefinition,
        rawWorkoutDoc: loadedDefinition.rawWorkoutDoc ?? selectedDefinition.rawWorkoutDoc,
        intervals: selectedDefinition.intervals,
        segments: selectedDefinition.segments,
        summary: selectedDefinition.summary,
    };
}
