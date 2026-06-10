use crate::domain::intervals::DateRange;

use super::{BoxFuture, CalendarLabel, CalendarLabelError, CalendarLabelSource};

#[derive(Clone)]
pub struct CompositeCalendarLabelSource<A, B>
where
    A: CalendarLabelSource + Clone + 'static,
    B: CalendarLabelSource + Clone + 'static,
{
    primary: A,
    secondary: B,
}

impl<A, B> CompositeCalendarLabelSource<A, B>
where
    A: CalendarLabelSource + Clone + 'static,
    B: CalendarLabelSource + Clone + 'static,
{
    pub fn new(primary: A, secondary: B) -> Self {
        Self { primary, secondary }
    }
}

impl<A, B> CalendarLabelSource for CompositeCalendarLabelSource<A, B>
where
    A: CalendarLabelSource + Clone + 'static,
    B: CalendarLabelSource + Clone + 'static,
{
    fn list_labels(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<CalendarLabel>, CalendarLabelError>> {
        let source = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let mut labels = source.primary.list_labels(&user_id, &range).await?;
            labels.extend(source.secondary.list_labels(&user_id, &range).await?);
            Ok(labels)
        })
    }
}
