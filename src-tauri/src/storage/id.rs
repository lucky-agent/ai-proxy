use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// 分配下一个请求 ID（单调递增，线程安全）。
pub(crate) fn next_request_id() -> u64 {
    NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
}

/// 启动时从 DB 最大 id 恢复计数器，避免重启后 ID 重复。
pub(crate) fn init_request_counter(max_id: u64) {
    NEXT_REQUEST_ID.store(max_id.wrapping_add(1), Ordering::Relaxed);
}
