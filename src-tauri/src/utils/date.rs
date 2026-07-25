use std::time::Instant;
use time::OffsetDateTime;

/// 统一的日期时间工具，所有时间戳获取都经由此模块。

/// 当前 UTC 毫秒时间戳，用于排序、展示和存储。
#[inline]
pub(crate) fn now_ms() -> i64 {
    let now = OffsetDateTime::now_utc();
    now.unix_timestamp() * 1000 + now.millisecond() as i64
}

/// 单调时钟计时起点，用于测量耗时（不受系统时间调整影响）。
#[inline]
pub(crate) fn instant_now() -> Instant {
    Instant::now()
}
