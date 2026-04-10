use std::collections::BTreeMap;

use crate::domain::intervals::DateRange;

use super::{
    BoxFuture, CalendarLabelError, CalendarLabelSource, CalendarLabelsResponse,
    CalendarLabelsUseCases,
};

#[derive(Clone)]
pub struct CalendarLabelsService<Source>
where
    Source: CalendarLabelSource + Clone + 'static,
{
    source: Source,
}

impl<Source> CalendarLabelsService<Source>
where
    Source: CalendarLabelSource + Clone + 'static,
{
    pub fn new(source: Source) -> Self {
        Self { source }
    }

    async fn list_labels_impl(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> Result<CalendarLabelsResponse, CalendarLabelError> {
        let labels = self.source.list_labels(user_id, range).await?;
        let mut labels_by_date = BTreeMap::new();

        for label in labels {
            let date = label.date.clone();
            let key = label.label_key.clone();
            labels_by_date
                .entry(date)
                .or_insert_with(BTreeMap::new)
                .insert(key, label);
        }

        Ok(CalendarLabelsResponse { labels_by_date })
    }
}

impl<Source> CalendarLabelsUseCases for CalendarLabelsService<Source>
where
    Source: CalendarLabelSource + Clone + 'static,
{
    fn list_labels(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<CalendarLabelsResponse, CalendarLabelError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move { service.list_labels_impl(&user_id, &range).await })
    }
}
