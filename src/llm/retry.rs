use crate::error::AppError;
use crate::monitor::{LogStatus, Monitor, TranslationLog, TranslationStage};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct RetryContext {
    pub max_retries: u32,
    pub retry_delay_secs: u64,
    pub current_attempt: u32,
    pub last_error: Option<AppError>,
    pub feed_name: String,
    pub article_title: String,
    pub model: String,
    pub monitor: Arc<RwLock<Monitor>>,
    pub current_log_id: Option<String>,
}

impl RetryContext {
    pub fn new(
        max_retries: u32,
        retry_delay_secs: u64,
        feed_name: String,
        article_title: String,
        model: String,
        monitor: Arc<RwLock<Monitor>>,
    ) -> Self {
        RetryContext {
            max_retries,
            retry_delay_secs,
            current_attempt: 0,
            last_error: None,
            feed_name,
            article_title,
            model,
            monitor,
            current_log_id: None,
        }
    }

    pub fn should_retry(&self) -> bool {
        self.current_attempt < self.max_retries
    }

    pub async fn record_failure(&mut self, error: AppError) {
        self.last_error = Some(error);
        if let Some(log_id) = &self.current_log_id {
            let error_msg = self
                .last_error
                .as_ref()
                .map(|e| e.to_string())
                .unwrap_or_default();
            self.monitor
                .write()
                .await
                .update_log(&self.feed_name, log_id, |log| {
                    log.status = LogStatus::Failed(error_msg);
                });
        }
    }

    pub async fn prepare_retry(&mut self) {
        self.current_attempt += 1;
        let log = TranslationLog {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            article_title: self.article_title.clone(),
            stage: TranslationStage::TranslateAndSummarize,
            status: LogStatus::Started,
            model: self.model.clone(),
            prompt_tokens: None,
            completion_tokens: None,
            streamed_text: String::new(),
        };
        self.current_log_id = Some(log.id.clone());
        self.monitor.write().await.add_log(&self.feed_name, log);
    }

    pub async fn wait(&self) {
        if self.retry_delay_secs > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(self.retry_delay_secs)).await;
        }
    }

    pub async fn mark_success(&self, usage: &crate::llm::UsageInfo) {
        if let Some(log_id) = &self.current_log_id {
            self.monitor
                .write()
                .await
                .update_log(&self.feed_name, log_id, |log| {
                    log.status = LogStatus::Completed;
                    log.prompt_tokens = Some(usage.prompt_tokens);
                    log.completion_tokens = Some(usage.completion_tokens);
                });
            self.monitor
                .write()
                .await
                .add_token_usage(
                    &self.feed_name,
                    &self.model,
                    usage.prompt_tokens,
                    usage.completion_tokens,
                );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(max_retries: u32) -> RetryContext {
        RetryContext::new(
            max_retries,
            0,
            "test-feed".into(),
            "Test Article".into(),
            "gpt-4".into(),
            Arc::new(RwLock::new(Monitor::new())),
        )
    }

    #[test]
    fn new_context_starts_at_zero() {
        let ctx = make_ctx(3);
        assert_eq!(ctx.current_attempt, 0);
        assert!(ctx.last_error.is_none());
        assert!(ctx.current_log_id.is_none());
    }

    #[test]
    fn should_retry_when_under_limit() {
        let ctx = make_ctx(3);
        assert!(ctx.should_retry());
    }

    #[test]
    fn should_not_retry_at_limit() {
        let mut ctx = make_ctx(2);
        ctx.current_attempt = 2;
        assert!(!ctx.should_retry());
    }

    #[test]
    fn should_retry_with_zero_max() {
        let ctx = make_ctx(0);
        assert!(!ctx.should_retry());
    }

    #[tokio::test]
    async fn prepare_retry_increments_attempt() {
        let mut ctx = make_ctx(3);
        ctx.prepare_retry().await;
        assert_eq!(ctx.current_attempt, 1);
        assert!(ctx.current_log_id.is_some());
    }

    #[tokio::test]
    async fn prepare_retry_adds_log_to_monitor() {
        let mut ctx = make_ctx(3);
        ctx.prepare_retry().await;
        let monitor = ctx.monitor.read().await;
        let logs = monitor.get_logs("test-feed");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].article_title, "Test Article");
        assert_eq!(logs[0].model, "gpt-4");
        assert!(matches!(logs[0].status, LogStatus::Started));
    }

    #[tokio::test]
    async fn record_failure_updates_log_status() {
        let mut ctx = make_ctx(3);
        ctx.prepare_retry().await;
        ctx.record_failure(AppError::Llm("test error".into())).await;
        let monitor = ctx.monitor.read().await;
        let logs = monitor.get_logs("test-feed");
        assert_eq!(logs.len(), 1);
        match &logs[0].status {
            LogStatus::Failed(msg) => assert_eq!(msg, "llm api error: test error"),
            _ => panic!("expected Failed status"),
        }
    }

    #[tokio::test]
    async fn record_failure_without_log_does_not_panic() {
        let mut ctx = make_ctx(3);
        ctx.record_failure(AppError::Llm("test error".into())).await;
        assert!(ctx.last_error.is_some());
    }

    #[tokio::test]
    async fn mark_success_updates_log_and_tokens() {
        let mut ctx = make_ctx(3);
        ctx.prepare_retry().await;
        let usage = crate::llm::UsageInfo {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };
        ctx.mark_success(&usage).await;
        let monitor = ctx.monitor.read().await;
        let logs = monitor.get_logs("test-feed");
        assert_eq!(logs.len(), 1);
        assert!(matches!(logs[0].status, LogStatus::Completed));
        assert_eq!(logs[0].prompt_tokens, Some(100));
        assert_eq!(logs[0].completion_tokens, Some(50));
    }

    #[tokio::test]
    async fn mark_success_without_log_does_not_panic() {
        let ctx = make_ctx(3);
        let usage = crate::llm::UsageInfo {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };
        ctx.mark_success(&usage).await;
    }

    #[tokio::test]
    async fn multiple_retries_create_separate_logs() {
        let mut ctx = make_ctx(3);
        ctx.prepare_retry().await;
        let first_log_id = ctx.current_log_id.clone();
        ctx.record_failure(AppError::Llm("first failure".into())).await;

        ctx.prepare_retry().await;
        let second_log_id = ctx.current_log_id.clone();
        assert_ne!(first_log_id, second_log_id);

        let monitor = ctx.monitor.read().await;
        let logs = monitor.get_logs("test-feed");
        assert_eq!(logs.len(), 2);
    }

    #[tokio::test]
    async fn wait_with_zero_delay_returns_immediately() {
        let ctx = make_ctx(3);
        let start = std::time::Instant::now();
        ctx.wait().await;
        assert!(start.elapsed() < std::time::Duration::from_millis(100));
    }
}
