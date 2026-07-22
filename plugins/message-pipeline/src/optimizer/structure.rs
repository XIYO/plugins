use crate::model::NormalizedMessage;

use super::OptimizedMessage;

const DUPLICATE_WINDOW_SECONDS: i64 = 300;

pub(super) fn is_consecutive_duplicate(
    previous: &OptimizedMessage,
    current: &NormalizedMessage,
    current_content: &str,
) -> bool {
    previous.original.source == current.source
        && previous.original.source_thread_id == current.source_thread_id
        && previous.original.source_author_id == current.source_author_id
        && previous.content == current_content
        && (0..=DUPLICATE_WINDOW_SECONDS)
            .contains(&(current.timestamp_utc - previous.original.timestamp_utc).num_seconds())
}
