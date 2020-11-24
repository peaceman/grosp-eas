use std::error::Error as StdError;
use std::fmt;
use tracing::error;
use tracing_error::SpanTrace;

#[derive(Debug, thiserror::Error)]
pub struct Error {
    source: ErrorKind,
    span_trace: SpanTrace,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error(transparent)]
    Fatal(anyhow::Error),
    #[error(transparent)]
    NonFatal(#[from] anyhow::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.source, fmt)
    }
}

impl<E> From<E> for Error
where
    ErrorKind: From<E>,
{
    fn from(source: E) -> Self {
        Self {
            source: ErrorKind::from(source),
            span_trace: SpanTrace::capture(),
        }
    }
}

pub fn handle_error(error: Box<dyn StdError + Send + Sync>) -> bool {
    let (error, stop_actor, span_trace) = match error.downcast_ref::<Error>() {
        Some(e) => (
            format!("{:?}", e.source),
            matches!(&e.source, ErrorKind::Fatal(_)),
            Some(&e.span_trace),
        ),
        None => (format!("{:?}", error), false, None),
    };

    error!(
        %stop_actor,
        "ActorError: {} SpanTrace: {}",
        error,
        span_trace
            .map(|st| format!("{}", st))
            .unwrap_or_else(|| String::from("None"))
    );

    stop_actor
}
