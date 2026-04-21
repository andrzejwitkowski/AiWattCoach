use super::*;

impl<Tasks, Workers, Time> TaskSchedulerService<Tasks, Workers, Time>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    pub(super) async fn subscribe_to_task_updates(&self, task_id: &str) -> TaskWatchReceiver {
        let mut waiters = self.task_waiters.lock().await;
        waiters
            .entry(task_id.to_string())
            .or_insert_with(|| {
                let (sender, _) = watch::channel(None);
                sender
            })
            .subscribe()
    }

    pub(super) async fn publish_task_update(&self, task: ScheduledTask) {
        let sender = {
            let mut waiters = self.task_waiters.lock().await;
            waiters
                .entry(task.id.clone())
                .or_insert_with(|| {
                    let (sender, _) = watch::channel(None);
                    sender
                })
                .clone()
        };
        let _ = sender.send(Some(task));
    }

    pub(super) async fn publish_terminal_task_update(&self, task: Option<ScheduledTask>) {
        let Some(task) = task else {
            return;
        };

        let sender = {
            let mut waiters = self.task_waiters.lock().await;
            waiters.remove(&task.id)
        };

        if let Some(sender) = sender {
            let _ = sender.send(Some(task));
        }
    }

    pub(super) async fn cleanup_task_waiter_if_unused(&self, task_id: &str) {
        let mut waiters = self.task_waiters.lock().await;
        let should_remove = waiters
            .get(task_id)
            .is_some_and(|sender| sender.receiver_count() == 0);
        if should_remove {
            waiters.remove(task_id);
        }
    }
}
