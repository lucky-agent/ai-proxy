use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tauri::ipc::Channel;

use crate::proxy::events::ProxyEvent;

use super::normalize::{AiContentBlock, AiConversation, AiTurn, JsonValueExt};
use super::session::SessionStore;
use super::{Provider, StreamState};

// ══════════════════════════════════════════════════════════════════════════════
// 常量
// ══════════════════════════════════════════════════════════════════════════════

/// 流式节流累积阈值（SSE data 累计字节数）；超过即推一次增量快照。
const AI_EMIT_THRESHOLD: usize = 160;
/// 非 JSON 响应 fallback 时的最大文本截取长度。
const AI_FALLBACK_TEXT_LIMIT: usize = 4096;

// ══════════════════════════════════════════════════════════════════════════════
// AiState
// ══════════════════════════════════════════════════════════════════════════════

/// AI 解析子状态：仅在请求命中 AI 检测规则时存在。
pub(crate) struct AiState {
    provider: Provider,
    session_id: String,
    sessions: Option<Arc<Mutex<SessionStore>>>,
    start_ms: i64,
    /// 流式状态机。`None` 表示非流式响应——EOS 时从完整 body 解析。
    stream_state: Option<Box<dyn StreamState>>,
    /// 覆盖率巡检：上游原始响应 JSON 的叶子字段名集合。
    raw_keys: HashSet<String>,
    /// 流式节流计数器（累计 SSE data 长度）；非流式恒为 0。
    stream_acc: usize,
    /// 请求侧归一化 turns，供 AiNormalized 事件自包含。
    request_turns: Vec<AiTurn>,
}

impl AiState {
    pub(crate) fn new(
        provider: Provider,
        session_id: String,
        sessions: Option<Arc<Mutex<SessionStore>>>,
        start_ms: i64,
        is_sse: bool,
        request_turns: Vec<AiTurn>,
    ) -> Self {
        let (stream_state, raw_keys, stream_acc) = if is_sse {
            (Some(provider.create_stream_state()), HashSet::new(), 0usize)
        } else {
            (None, HashSet::new(), 0usize)
        };
        Self {
            provider,
            session_id,
            sessions,
            start_ms,
            stream_state,
            raw_keys,
            stream_acc,
            request_turns,
        }
    }

    /// SSE 路径：消费分帧后的 events — JSON 解析 → 状态机驱动 → 节流快照。
    pub(crate) fn consume_sse(
        &mut self,
        events: &[crate::proxy::sse::SseEvent],
        request_id: u64,
        first_chunk_at: Option<i64>,
        sender: &Option<Channel<ProxyEvent>>,
    ) {
        let Some(ref mut state) = self.stream_state else {
            return;
        };
        for ev in events {
            if let Some(ref data) = ev.data
                && let Ok(value) = serde_json::from_str::<Value>(data)
            {
                self.raw_keys.extend(value.leaf_keys());
                state.apply(&ev.event, &value);
            }
        }
        self.stream_acc += sse_data_len(events);
        if self.stream_acc >= AI_EMIT_THRESHOLD {
            self.stream_acc = 0;
            let mut snap = state.snapshot();
            snap.start_ms = Some(self.start_ms);
            snap.first_chunk_ms = first_chunk_at.map(|at| (at - self.start_ms).max(0) as u64);
            emit_ai_normalized(
                sender,
                request_id,
                &self.session_id,
                snap,
                &self.request_turns,
            );
        }
    }

    /// EOS 收尾：流式 finalize + 快照 / 非流式 JSON 解析 → 覆盖率 → emit + commit。
    /// `body_buf` 仅在非流式路径有效。
    pub(crate) fn finalize(
        self,
        request_id: u64,
        status: u16,
        first_chunk_at: Option<i64>,
        sender: &Option<Channel<ProxyEvent>>,
        body_buf: Option<String>,
    ) {
        let (raw_keys, conv) = match self.stream_state {
            Some(mut state) => {
                state.finalize();
                let mut snap = state.snapshot();
                snap.first_chunk_ms = first_chunk_at.map(|at| (at - self.start_ms).max(0) as u64);
                (Some(self.raw_keys), Some(snap))
            }
            None => {
                let buf = body_buf.unwrap_or_default();
                let (raw_keys, conv) = match serde_json::from_str::<Value>(&buf) {
                    Ok(root) => {
                        let raw_keys = root.leaf_keys();
                        let conv = self.provider.parse_response_body(&root).or_else(|| {
                            log::warn!(
                                "[ai] unparsed non-streaming body for request {request_id} \
                                 (status {status}, {} bytes)",
                                buf.len()
                            );
                            Some(fallback_conversation(self.provider, &buf, status))
                        });
                        (Some(raw_keys), conv)
                    }
                    Err(e) => {
                        log::warn!(
                            "[ai] non-JSON response body for request {request_id} \
                             (status {status}, {} bytes): {e}",
                            buf.len()
                        );
                        (
                            None,
                            Some(fallback_conversation(self.provider, &buf, status)),
                        )
                    }
                };
                (raw_keys, conv)
            }
        };

        if let Some(mut conv) = conv {
            conv.start_ms = Some(self.start_ms);
            conv.duration_ms = Some((crate::utils::date::now_ms() - self.start_ms).max(0) as u64);

            // 覆盖率巡检
            if let Some(raw) = raw_keys {
                let ir_keys = serde_json::to_value(&conv)
                    .ok()
                    .map(|v| v.leaf_keys())
                    .unwrap_or_default();
                let mut uncovered: Vec<&str> =
                    raw.difference(&ir_keys).map(String::as_str).collect();
                uncovered.sort_unstable();
                if !uncovered.is_empty() {
                    log::info!(
                        "[ai-coverage] req#{} {} ({}) uncovered: {:?}",
                        request_id,
                        conv.model.as_deref().unwrap_or("-"),
                        conv.provider,
                        uncovered
                    );
                }
            }

            // 助理 turns 写入后端 SessionStore（供 prefix 匹配），
            // 不再通过 AiTimelineDelta 推前端——前端从 AiNormalized.conversation 自包含消费。
            let assistant_turns: Vec<AiTurn> = conv.turns.clone();
            if let Some(ref sessions) = self.sessions {
                let mut store = sessions.lock().expect("sessions lock");
                store.append_assistant_turns(&self.session_id, request_id, &assistant_turns);
            }

            emit_ai_normalized(
                sender,
                request_id,
                &self.session_id,
                conv.clone(),
                &self.request_turns,
            );
            commit_ai_final(sender, &self.sessions, request_id, &self.session_id, &conv);
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 公共辅助函数
// ══════════════════════════════════════════════════════════════════════════════

pub(crate) fn emit_ai_normalized(
    sender: &Option<Channel<ProxyEvent>>,
    request_id: u64,
    session_id: &str,
    conv: AiConversation,
    request_turns: &[AiTurn],
) {
    if let Some(ch) = sender {
        let _ = ch.send(ProxyEvent::AiNormalized {
            id: request_id,
            session_id: session_id.to_string(),
            provider: conv.provider.clone(),
            streaming: conv.streaming,
            conversation: conv,
            request_turns: request_turns.to_vec(),
        });
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 内部辅助函数
// ══════════════════════════════════════════════════════════════════════════════

fn commit_ai_final(
    sender: &Option<Channel<ProxyEvent>>,
    sessions: &Option<Arc<Mutex<SessionStore>>>,
    request_id: u64,
    session_id: &str,
    conv: &AiConversation,
) {
    let Some(sessions) = sessions else {
        return;
    };
    let mut store = sessions.lock().expect("sessions lock");
    store.refine_title(session_id, request_id, conv);
    if let Some(usage) = conv.usage.as_ref() {
        store.add_usage(session_id, usage);
    }
    if let Some(entry) = store.get(session_id)
        && let Some(ch) = sender
    {
        let _ = ch.send(ProxyEvent::AiSession {
            session_id: session_id.to_string(),
            scope_host: entry.scope.1.clone(),
            request_ids: entry.request_ids.clone(),
            usage_total: entry.usage_total.clone(),
            match_reason: entry.match_reason.clone(),
            title: entry.title.clone(),
            source: entry.source.clone(),
        });
    }
}

fn fallback_conversation(provider: Provider, body: &str, status: u16) -> AiConversation {
    let mut end = body.len().min(AI_FALLBACK_TEXT_LIMIT);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    AiConversation::new(
        provider.as_str(),
        vec![AiTurn::new(
            "assistant",
            vec![AiContentBlock::text(&body[..end])],
        )],
        false,
        None,
        None,
        Some(format!("http_{status}")),
    )
}

/// SSE events 总 data 长度（用于节流计数）。
fn sse_data_len(events: &[crate::proxy::sse::SseEvent]) -> usize {
    events
        .iter()
        .map(|ev| ev.data.as_ref().map_or(0, |d| d.len()))
        .sum()
}
