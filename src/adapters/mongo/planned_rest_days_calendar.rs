use crate::domain::{
    calendar_labels::{
        BoxFuture as CalendarLabelsBoxFuture, CalendarLabel, CalendarLabelError,
        CalendarLabelPayload, CalendarPlannedRestDayLabel,
    },
    intervals::DateRange,
    planned_rest_days::{expand_inclusive_date_range, PlannedRestDayRepository},
};

#[derive(Clone)]
pub struct MongoPlannedRestDayCalendarLabelSource<Repository>
where
    Repository: PlannedRestDayRepository + Clone + 'static,
{
    repository: Repository,
}

impl<Repository> MongoPlannedRestDayCalendarLabelSource<Repository>
where
    Repository: PlannedRestDayRepository + Clone + 'static,
{
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }
}

impl<Repository> crate::domain::calendar_labels::CalendarLabelSource
    for MongoPlannedRestDayCalendarLabelSource<Repository>
where
    Repository: PlannedRestDayRepository + Clone + 'static,
{
    fn list_labels(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> CalendarLabelsBoxFuture<Result<Vec<CalendarLabel>, CalendarLabelError>> {
        let source = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let entries = source
                .repository
                .list_intersecting_range(&user_id, &range)
                .await
                .map_err(map_repository_error)?;

            let mut labels = Vec::new();
            for entry in entries {
                let dates = expand_inclusive_date_range(&entry.start_date, &entry.end_date)
                    .map_err(map_repository_error)?;
                for date in dates {
                    if date < range.oldest || date > range.newest {
                        continue;
                    }

                    labels.push(CalendarLabel {
                        label_key: format!("planned_rest_day:{}", entry.planned_rest_day_id),
                        date: date.clone(),
                        title: entry.display_title(),
                        subtitle: entry.label_subtitle_for_date(&date),
                        payload: CalendarLabelPayload::PlannedRestDay(
                            CalendarPlannedRestDayLabel {
                                planned_rest_day_id: entry.planned_rest_day_id.clone(),
                                start_date: entry.start_date.clone(),
                                end_date: entry.end_date.clone(),
                                title: entry.title.clone(),
                                note: entry.note.clone(),
                            },
                        ),
                    });
                }
            }

            Ok(labels)
        })
    }
}

fn map_repository_error(
    error: crate::domain::planned_rest_days::PlannedRestDayError,
) -> CalendarLabelError {
    CalendarLabelError::Internal(error.to_string())
}
