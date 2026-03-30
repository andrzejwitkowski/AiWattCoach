use reqwest::{Client, RequestBuilder, StatusCode};

use crate::domain::intervals::{Activity, IntervalsCredentials, IntervalsError};

use super::{
    errors::{map_api_error, map_connection_error},
    mapping::{
        map_activity_interval, map_activity_interval_group, map_activity_response,
        map_activity_stream, should_persist_stream,
    },
    ApiFailure, IntervalsIcuClient,
};
use crate::adapters::intervals_icu::dto::{
    ActivityIntervalsResponse, ActivityResponse, ActivityStreamResponse,
};

impl IntervalsIcuClient {
    pub(super) async fn request_activity(
        client: &Client,
        base_url: &str,
        credentials: &IntervalsCredentials,
        activity_id: &str,
        include_intervals: bool,
    ) -> Result<reqwest::Response, ApiFailure> {
        let url = Self::activity_url_impl(base_url, activity_id, "");
        let mut request = client
            .get(url)
            .basic_auth("API_KEY", Some(&credentials.api_key));

        if include_intervals {
            request = request.query(&[("intervals", "true")]);
        }

        Self::send_request(request).await
    }

    pub(super) async fn fetch_base_activity(
        client: &Client,
        base_url: &str,
        credentials: &IntervalsCredentials,
        activity_id: &str,
        include_intervals: bool,
    ) -> Result<Activity, IntervalsError> {
        let response = Self::request_activity(
            client,
            base_url,
            credentials,
            activity_id,
            include_intervals,
        )
        .await
        .map_err(|failure| match failure.status {
            Some(StatusCode::NOT_FOUND) => IntervalsError::NotFound,
            _ => failure.error,
        })?;
        let payload: ActivityResponse = response.json().await.map_err(map_api_error)?;
        Ok(map_activity_response(payload))
    }

    pub(super) async fn send_request(
        request: RequestBuilder,
    ) -> Result<reqwest::Response, ApiFailure> {
        let response = Self::with_trace_context(request)
            .send()
            .await
            .map_err(|error| ApiFailure {
                status: error.status(),
                error: map_connection_error(error),
                response_body: None,
            })?;

        if response.status().is_success() {
            return Ok(response);
        }

        Err(super::errors::map_error_response(response).await)
    }

    pub(super) async fn fetch_activity_details(
        client: Client,
        base_url: String,
        credentials: IntervalsCredentials,
        activity_id: String,
    ) -> Result<Activity, IntervalsError> {
        let intervals_url = Self::activity_url_impl(&base_url, &activity_id, "/intervals");
        let streams_url = Self::activity_url_impl(&base_url, &activity_id, "/streams");

        let mut activity =
            Self::fetch_base_activity(&client, &base_url, &credentials, &activity_id, false)
                .await?;
        let mut intervals_definitively_unavailable = false;
        let mut streams_definitively_unavailable = false;

        let intervals_result = Self::send_request(
            client
                .get(intervals_url)
                .basic_auth("API_KEY", Some(&credentials.api_key)),
        )
        .await;

        match intervals_result {
            Ok(intervals_response) => {
                match intervals_response.json::<ActivityIntervalsResponse>().await {
                    Ok(intervals) => {
                        activity.details.intervals = intervals
                            .icu_intervals
                            .into_iter()
                            .map(map_activity_interval)
                            .collect();
                        activity.details.interval_groups = intervals
                            .icu_groups
                            .into_iter()
                            .map(map_activity_interval_group)
                            .collect();
                        intervals_definitively_unavailable = true;
                    }
                    Err(error) => {
                        let error = map_api_error(error);
                        tracing::warn!(
                            activity_id,
                            %error,
                            "intervals enrichment payload could not be parsed; returning base activity without intervals"
                        );
                    }
                }
            }
            Err(failure) => {
                tracing::warn!(
                    activity_id,
                    %failure.error,
                    response_body = failure.response_body.as_deref().unwrap_or(""),
                    "intervals enrichment failed; returning base activity without intervals"
                );

                if failure.is_unprocessable_entity() {
                    match Self::fetch_base_activity(
                        &client,
                        &base_url,
                        &credentials,
                        &activity_id,
                        true,
                    )
                    .await
                    {
                        Ok(fallback_activity) => {
                            if !fallback_activity.details.intervals.is_empty() {
                                activity.details.intervals = fallback_activity.details.intervals;
                            }
                            if !fallback_activity.details.interval_groups.is_empty() {
                                activity.details.interval_groups =
                                    fallback_activity.details.interval_groups;
                            }
                            intervals_definitively_unavailable = true;
                        }
                        Err(error) => {
                            tracing::warn!(
                                activity_id,
                                %error,
                                "intervals=true fallback fetch failed; returning base activity without intervals"
                            );
                        }
                    }
                }
            }
        }

        let streams_result = if activity.stream_types.is_empty() {
            Self::send_request(
                client
                    .get(streams_url)
                    .basic_auth("API_KEY", Some(&credentials.api_key))
                    .query(&[("includeDefaults", "true")]),
            )
            .await
        } else {
            let mut query_params = Vec::with_capacity(activity.stream_types.len() + 1);
            for stream_type in &activity.stream_types {
                query_params.push(("types", stream_type.clone()));
            }
            query_params.push(("includeDefaults", "true".to_string()));

            Self::send_request(
                client
                    .get(streams_url)
                    .basic_auth("API_KEY", Some(&credentials.api_key))
                    .query(&query_params),
            )
            .await
        };

        match streams_result {
            Ok(streams_response) => {
                match streams_response.json::<Vec<ActivityStreamResponse>>().await {
                    Ok(streams) => {
                        activity.details.streams = streams
                            .into_iter()
                            .filter(should_persist_stream)
                            .map(map_activity_stream)
                            .collect();
                        streams_definitively_unavailable = true;
                    }
                    Err(error) => {
                        let error = map_api_error(error);
                        tracing::warn!(
                            activity_id,
                            %error,
                            "streams enrichment payload could not be parsed; returning base activity without streams"
                        );
                    }
                }
            }
            Err(failure) => {
                tracing::warn!(
                    activity_id,
                    %failure.error,
                    response_body = failure.response_body.as_deref().unwrap_or(""),
                    "streams enrichment failed; returning base activity without streams"
                );

                if failure.is_unprocessable_entity() {
                    streams_definitively_unavailable = true;
                }
            }
        }

        if activity.source.as_deref() == Some("STRAVA")
            && intervals_definitively_unavailable
            && streams_definitively_unavailable
            && activity.details.intervals.is_empty()
            && activity.details.interval_groups.is_empty()
            && activity.details.streams.is_empty()
        {
            activity.details_unavailable_reason = Some(
                "Intervals.icu did not provide detailed data for this imported activity."
                    .to_string(),
            );
        }

        Ok(activity)
    }
}
