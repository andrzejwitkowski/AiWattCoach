use std::future::Future;

use crate::domain::workout_summary::PublicToolCall;

pub(crate) async fn materialize_public_tool_calls_idempotently<
    E,
    AlreadyFn,
    AlreadyFuture,
    AppendFn,
    AppendFuture,
>(
    existing_ids: Vec<String>,
    public_tool_calls: &[PublicToolCall],
    already_materialized: AlreadyFn,
    append: AppendFn,
) -> Result<Vec<String>, E>
where
    AlreadyFn: Fn(&str) -> AlreadyFuture,
    AlreadyFuture: Future<Output = Result<bool, E>>,
    AppendFn: Fn(PublicToolCall) -> AppendFuture,
    AppendFuture: Future<Output = Result<(), E>>,
{
    let mut materialized_ids = existing_ids;

    for tool_call in public_tool_calls {
        if materialized_ids.iter().any(|id| id == &tool_call.id) {
            continue;
        }

        if already_materialized(&tool_call.id).await? {
            materialized_ids.push(tool_call.id.clone());
            continue;
        }

        append(tool_call.clone()).await?;
        materialized_ids.push(tool_call.id.clone());
    }

    Ok(materialized_ids)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::Mutex;

    use crate::domain::workout_summary::PublicToolCall;

    fn tool_call(id: &str, name: &str) -> PublicToolCall {
        PublicToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments_json: "{}".to_string(),
            arguments_preview: None,
        }
    }

    #[tokio::test]
    async fn materialize_public_tool_calls_idempotently_skips_append_when_id_already_present() {
        let appended = Arc::new(Mutex::new(Vec::<String>::new()));
        let calls = vec![tool_call("tool-1", "first")];

        let result = super::materialize_public_tool_calls_idempotently(
            vec!["tool-1".to_string()],
            &calls,
            |_| async { Ok::<bool, String>(false) },
            {
                let appended = appended.clone();
                move |tool_call| {
                    let appended = appended.clone();
                    async move {
                        appended.lock().await.push(tool_call.id);
                        Ok::<(), String>(())
                    }
                }
            },
        )
        .await
        .expect("existing ids should not fail");

        assert_eq!(result, vec!["tool-1".to_string()]);
        assert!(appended.lock().await.is_empty());
    }

    #[tokio::test]
    async fn materialize_public_tool_calls_idempotently_records_existing_message_without_append() {
        let appended = Arc::new(Mutex::new(Vec::<String>::new()));
        let calls = vec![tool_call("tool-1", "first")];

        let result = super::materialize_public_tool_calls_idempotently(
            Vec::new(),
            &calls,
            |tool_call_id| {
                let tool_call_id = tool_call_id.to_string();
                async move { Ok::<bool, String>(tool_call_id == "tool-1") }
            },
            {
                let appended = appended.clone();
                move |tool_call| {
                    let appended = appended.clone();
                    async move {
                        appended.lock().await.push(tool_call.id);
                        Ok::<(), String>(())
                    }
                }
            },
        )
        .await
        .expect("existing stored message should not fail");

        assert_eq!(result, vec!["tool-1".to_string()]);
        assert!(appended.lock().await.is_empty());
    }

    #[tokio::test]
    async fn materialize_public_tool_calls_idempotently_appends_missing_call_once() {
        let appended = Arc::new(Mutex::new(Vec::<String>::new()));
        let calls = vec![tool_call("tool-1", "first")];

        let result = super::materialize_public_tool_calls_idempotently(
            Vec::new(),
            &calls,
            |_| async { Ok::<bool, String>(false) },
            {
                let appended = appended.clone();
                move |tool_call| {
                    let appended = appended.clone();
                    async move {
                        appended.lock().await.push(tool_call.id);
                        Ok::<(), String>(())
                    }
                }
            },
        )
        .await
        .expect("missing call should append once");

        assert_eq!(result, vec!["tool-1".to_string()]);
        assert_eq!(appended.lock().await.clone(), vec!["tool-1".to_string()]);
    }

    #[tokio::test]
    async fn materialize_public_tool_calls_idempotently_handles_mixed_calls_in_order() {
        let appended = Arc::new(Mutex::new(Vec::<String>::new()));
        let calls = vec![
            tool_call("tool-1", "first"),
            tool_call("tool-2", "second"),
            tool_call("tool-3", "third"),
        ];

        let result = super::materialize_public_tool_calls_idempotently(
            vec!["tool-1".to_string()],
            &calls,
            |tool_call_id| {
                let tool_call_id = tool_call_id.to_string();
                async move { Ok::<bool, String>(tool_call_id == "tool-2") }
            },
            {
                let appended = appended.clone();
                move |tool_call| {
                    let appended = appended.clone();
                    async move {
                        appended.lock().await.push(tool_call.id);
                        Ok::<(), String>(())
                    }
                }
            },
        )
        .await
        .expect("mixed calls should materialize cleanly");

        assert_eq!(
            result,
            vec![
                "tool-1".to_string(),
                "tool-2".to_string(),
                "tool-3".to_string()
            ]
        );
        assert_eq!(appended.lock().await.clone(), vec!["tool-3".to_string()]);
    }

    #[tokio::test]
    async fn materialize_public_tool_calls_idempotently_propagates_lookup_error() {
        let calls = vec![tool_call("tool-1", "first")];

        let error = super::materialize_public_tool_calls_idempotently(
            Vec::new(),
            &calls,
            |_| async { Err::<bool, String>("lookup failed".to_string()) },
            |_| async { Ok::<(), String>(()) },
        )
        .await
        .expect_err("lookup error should propagate");

        assert_eq!(error, "lookup failed".to_string());
    }

    #[tokio::test]
    async fn materialize_public_tool_calls_idempotently_propagates_append_error() {
        let calls = vec![tool_call("tool-1", "first")];

        let error = super::materialize_public_tool_calls_idempotently(
            Vec::new(),
            &calls,
            |_| async { Ok::<bool, String>(false) },
            |_| async { Err::<(), String>("append failed".to_string()) },
        )
        .await
        .expect_err("append error should propagate");

        assert_eq!(error, "append failed".to_string());
    }
}
