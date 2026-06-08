const CALENDAR_TRANSLATIONS: Record<string, string | ((options?: { count?: number; priority?: string; value?: string }) => string)> = {
  'calendar.performanceCalendar': 'Performance Calendar',
  'calendar.baseMonth': 'Base Month',
  'calendar.visibleWindow': 'Visible Window',
  'calendar.scrollMode': 'Scroll Mode',
  'calendar.infinite': 'Infinite',
  'calendar.noEvents': 'No events',
  'calendar.dayItems': 'Day items',
  'calendar.viewItems': (options) => `View ${options?.count ?? 0} items`,
  'calendar.closeDayItems': 'Close day items',
  'calendar.closeRaceDetails': 'Close race details',
  'calendar.closeWorkoutDetails': 'Close workout details',
  'calendar.mobilePreviousWeeks': 'Show previous weeks',
  'calendar.mobileNextWeeks': 'Show next weeks',
  'calendar.raceDay': 'Race Day',
  'calendar.plannedWorkout': 'Planned Workout',
  'calendar.completedWorkout': 'Completed Workout',
  'calendar.eventOther': 'Event',
  'calendar.raceDistance': 'Distance',
  'calendar.raceDiscipline': 'Discipline',
  'calendar.racePriority': 'Priority',
  'calendar.raceSyncStatus': 'Sync Status',
  'calendar.priorityLabel': (options) => `Cat. ${options?.priority ?? ''}`,
  'calendar.raceDisciplineRoad': 'Road',
  'races.distanceValue': (options) => `${options?.value ?? ''} km`,
  'races.syncStatus.synced': 'Synced',
};

export function calendarTranslationMock(
  key: string,
  options?: { count?: number; priority?: string; value?: string },
) {
  const translation = CALENDAR_TRANSLATIONS[key];
  if (typeof translation === 'function') {
    return translation(options);
  }
  return translation ?? key;
}
