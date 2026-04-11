use reqwest::{Client, RequestBuilder, StatusCode};

use crate::domain::intervals::{Activity, IntervalsCredentials, IntervalsError};

use super::{
    errors::{map_connection_error, summarize_log_body},
    logging::{execute_request, BodyLoggingMode, LoggedResponse},
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
    ) -> Result<LoggedResponse, ApiFailure> {
        let url = Self::activity_url_impl(base_url, activity_id, "");
        let mut request = client
            .get(url)
            .basic_auth("API_KEY", Some(&credentials.api_key));

        if include_intervals {
            request = request.query(&[("intervals", "true")]);
        }

        Self::send_request(client, request).await
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
        let payload: ActivityResponse = serde_json::from_slice(&response.body)
            .map_err(|error| IntervalsError::ApiError(error.to_string()))?;
        Ok(map_activity_response(payload))
    }

    pub(super) async fn send_request(
        client: &Client,
        request: RequestBuilder,
    ) -> Result<LoggedResponse, ApiFailure> {
        let logged = execute_request(
            client,
            Self::with_trace_context(request),
            BodyLoggingMode::None,
        )
        .await
        .map_err(|error| ApiFailure {
            status: error.status(),
            error: map_connection_error(error),
            response_body: None,
        })?;

        if logged.status.is_success() {
            return Ok(logged);
        }

        Err(super::errors::map_error_response_from_logged_response(
            logged,
        ))
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
            &client,
            client
                .get(intervals_url)
                .basic_auth("API_KEY", Some(&credentials.api_key)),
        )
        .await;

        match intervals_result {
            Ok(intervals_response) => {
                match serde_json::from_slice::<ActivityIntervalsResponse>(&intervals_response.body)
                {
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
                    }
                    Err(error) => {
                        tracing::warn!(
                            activity_id,
                            %error,
                            response_body = %summarize_log_body(&intervals_response.body),
                            "intervals enrichment payload could not be parsed; returning base activity without intervals"
                        );
                    }
                }
            }
            Err(failure) => {
                tracing::warn!(
                    activity_id,
                    %failure.error,
                    response_body = failure.response_body.as_deref().unwrap_or_default(),
                    "intervals enrichment failed; returning base activity without intervals"
                );

                if failure.is_unprocessable_entity() {
                    intervals_definitively_unavailable = true;
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
                &client,
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
                &client,
                client
                    .get(streams_url)
                    .basic_auth("API_KEY", Some(&credentials.api_key))
                    .query(&query_params),
            )
            .await
        };

        match streams_result {
            Ok(streams_response) => {
                match serde_json::from_slice::<Vec<ActivityStreamResponse>>(&streams_response.body)
                {
                    Ok(streams) => {
                        activity.details.streams = streams
                            .into_iter()
                            .filter(should_persist_stream)
                            .map(map_activity_stream)
                            .collect();
                    }
                    Err(error) => {
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
                    response_body = failure.response_body.as_deref().unwrap_or_default(),
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
