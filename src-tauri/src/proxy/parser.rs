use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use log::info;
use rama::http::{Body, Response, StatusCode, header};
use tauri::ipc::Channel;

use crate::config::AiRuleSource;
use crate::proxy::ctx::{ProxyCtx, collect_headers, map_to_kv_json};

use super::events::ProxyEvent;

/// Accepts body as raw bytes to avoid unnecessary UTF-8 allocation;
/// only converts lossily when emitting the RequestChunk event.
pub(crate) fn record_request(ctx: &ProxyCtx, body: &[u8]) {
    let query_params = ctx.query_params();
    let req_headers = ctx.header_map();

    let req_content_type = ctx.header(&header::CONTENT_TYPE).map(str::to_owned);
    let req_content_length = ctx.header_typed::<u64>(&header::CONTENT_LENGTH);

    let host_hint = ctx.header(&header::HOST);

    let (ai_hint, ai_sources) =
        crate::proxy::ai_hint::compute_ai_hint(ctx.uri(), host_hint, ctx.settings());

    ctx.send(ProxyEvent::Request {
        id: ctx.request_id().to_string(),
        method: ctx.method().to_string(),
        uri: ctx.uri().to_string(),
        timestamp: ctx.start_ms(),
        headers: req_headers.clone(),
        query_params: query_params.clone(),
        decrypted: true,
        content_type: req_content_type,
        content_length: req_content_length,
        ai_hint: ai_hint.clone(),
    });

    // body 只做一次 lossy 转换：DB 侧借用，事件侧末尾 move。
    let body_str = (!body.is_empty()).then(|| String::from_utf8_lossy(body).into_owned());

    // ── DB 持久化（仅解密流量；db 不为 None）──
    if let Some(db) = ctx.db_ref() {
        match db.upsert_traffic_log(
            ctx.method().as_str(),
            &ctx.uri().to_string(),
            crate::utils::date::now_ms(),
            &ctx.headers_json(),
            &ctx.query_json(),
            body_str.as_deref(),
        ) {
            Ok(id) => ctx.set_db_id(id),
            Err(e) => log::warn!("[db] upsert_traffic_log: {e:?}"),
        }
    }

    if let Some(chunk) = body_str {
        ctx.send(ProxyEvent::RequestChunk {
            id: ctx.request_id().to_string(),
            chunk,
        });
    }

    // ── AI 会话分组（归一化的副产品）──
    log_ai_request(ctx, &ai_hint, &ai_sources, host_hint, body);
}

/// 请求侧 AI 归一化 + 会话分组。仅 AI 流量执行；判定结果存 ctx 供响应侧消费。
/// `sources`：命中规则的 (来源, 合并头) 对，其合并头置顶于全局名单参与分组。
fn log_ai_request(
    ctx: &ProxyCtx,
    ai_hint: &super::events::AiHint,
    sources: &[AiRuleSource],
    host_hint: Option<&str>,
    body: &[u8],
) {
    use super::events::AiHint;

    // AI 检测总开关关闭 → 完全跳过，不检测/不归一化/不推事件。
    if !ctx.settings().ai.enabled {
        return;
    }

    // 非 AI 流量（未命中 URL 规则）直接跳过，零开销。
    let hint_provider = match ai_hint {
        AiHint::None => return,
        AiHint::Candidate => None,
        AiHint::Provider(p) => Some(p.as_str()),
    };
    let Some(sessions) = ctx.sessions() else {
        return;
    };
    let body_str = String::from_utf8_lossy(body);
    let Some(provider) = super::ai::provider_for_request(hint_provider, &body_str) else {
        return;
    };

    let turns = super::ai::parse_request(provider, &body_str);
    if turns.is_empty() {
        return;
    }

    let host = ctx
        .uri()
        .host_str()
        .as_deref()
        .or(host_hint)
        .unwrap_or("")
        .to_string();
    let cfg = &ctx.settings().ai.session;

    // 分组 + 会话快照一次锁内完成（AssignResult 自带快照，免二次加锁读表）。
    let session_headers = session_header_list(sources, &cfg.session_headers);
    let result = {
        let mut store = sessions.lock().expect("sessions lock");
        store.assign(
            provider,
            &host,
            &session_headers,
            ctx.header_map(),
            &turns,
            cfg.prefix_match_fallback,
            ctx.request_id(),
        )
    };
    // 请求侧先发一次快照：只带请求体，assistant 侧留空、streaming=true。
    // 与响应解耦——响应缺席/解析失败时请求体也能立即展示；响应到达后响应侧会
    // 再发一次 AiNormalized（同 id）覆盖此快照，补上 assistant 回复并定稿。
    let req_snapshot = super::ai::AiConversation {
        provider: provider.as_str().to_string(),
        turns: Vec::new(),
        streaming: true,
        model: None,
        usage: None,
        finish_reason: None,
    };
    let turns = Arc::new(turns);
    emit_ai_normalized(
        ctx.sender(),
        ctx.request_id(),
        &result.session_id,
        turns.as_ref().clone(),
        req_snapshot,
    );

    ctx.set_ai_req(provider, result.session_id.clone(), turns);
    // 保持事件顺序：AiNormalized 先于 AiSession。
    ctx.send(ProxyEvent::AiSession {
        session_id: result.session_id,
        scope_host: host,
        turn_count: result.request_ids.len() as u32,
        request_ids: result.request_ids,
        usage_total: result.usage_total,
        match_reason: result.match_reason,
        title: result.title,
        source: result.source,
    });
}

/// Handle direct (non-tunnel) requests to the proxy itself.
/// Constructs a Response directly from the request headers (held by ctx) and body.
pub(crate) fn direct_response(ctx: &ProxyCtx, body: Body) -> Response {
    let mut resp: Response = Response::builder()
        .status(StatusCode::OK)
        .body(body)
        .expect("valid status code and body for direct response");
    *resp.headers_mut() = ctx.headers().clone();
    record_response(ctx, resp)
}

/// Log response and emit via channel.
pub(crate) fn record_response(ctx: &ProxyCtx, resp: Response) -> Response {
    let status = resp.status();
    info!("Response [{} {}] {}", ctx.method(), ctx.uri(), status);

    let (parts, body) = resp.into_parts();

    let resp_headers: HashMap<String, String> = collect_headers(&parts.headers);

    let resp_content_type = resp_headers.get(header::CONTENT_TYPE.as_str()).cloned();
    let resp_content_length = resp_headers
        .get(header::CONTENT_LENGTH.as_str())
        .and_then(|s| s.parse::<u64>().ok());

    let is_sse = resp_content_type
        .as_deref()
        .map(|ct| ct.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false);

    let now_ms = crate::utils::date::now_ms();
    // max(0)：时钟回拨时避免负数被 as u64 变成天文数字
    let duration_ms = (now_ms - ctx.start_ms()).max(0) as u64;

    ctx.send(ProxyEvent::Response {
        id: ctx.request_id().to_string(),
        status: status.as_u16(),
        timestamp: now_ms,
        duration_ms,
        headers: resp_headers.clone(),
        content_type: resp_content_type,
        content_length: resp_content_length,
    });

    // ── DB 更新响应元数据 ──
    if let (Some(db), Some(db_id)) = (ctx.db_ref(), ctx.db_id()) {
        let h_json = map_to_kv_json(&resp_headers);
        db.update_traffic_response(db_id, status.as_u16(), now_ms, duration_ms, &h_json)
            .ok();
    }

    // 读取body后resp会被消费掉
    let logged_body = log_body_chunks(body, ctx, is_sse);

    Response::from_parts(parts, logged_body)
}

/// 响应侧 AI 归一化模式。请求侧判定为 AI 时才启用。
enum AiRespMode {
    /// 流式：有状态增量 SSE 解析器。
    Streaming(super::ai::stream::StreamParser),
    /// 非流式：累积完整 body，末尾一次性解析。
    NonStreaming(String),
}

/// 流式增量推送节流阈值（累积字符数）。
const AI_EMIT_THRESHOLD: usize = 160;

/// 追加 chunk 到累积缓冲，总量超过 [`BODY_CAPTURE_LIMIT`] 后停止（DB 只存前缀）。
/// 截断处按 UTF-8 字符边界回退，避免 panic。
fn push_capped(buf: &mut String, chunk: &str) {
    use crate::utils::buf_pool::BODY_CAPTURE_LIMIT;
    let room = BODY_CAPTURE_LIMIT.saturating_sub(buf.len());
    if room == 0 {
        return;
    }
    if chunk.len() <= room {
        buf.push_str(chunk);
    } else {
        let mut end = room;
        while !chunk.is_char_boundary(end) {
            end -= 1;
        }
        buf.push_str(&chunk[..end]);
    }
}

/// 逐 chunk 观测状态 + 流结束定稿逻辑。
/// 由透传流（正常流结束）与 `on_drop`（客户端中途断开）共享，
/// [`Self::finalize`] 幂等，保证只执行一次。
struct BodyObserver {
    request_id: String,
    sender: Option<Channel<ProxyEvent>>,
    /// 请求侧判定的 (provider, session_id, request_turns)；None 表示非 AI 流量，零开销。
    /// turns 为 Arc 共享（与 ctx 同一份），克隆仅复制指针。
    ai_req: Option<(super::ai::Provider, String, Arc<Vec<super::ai::AiTurn>>)>,
    sessions: Option<Arc<Mutex<super::ai::session::SessionStore>>>,
    /// DB 持久化：流结束时写入 response_body。
    db: Option<Arc<crate::config::db::Db>>,
    db_id: Option<i64>,
    ai_mode: Option<AiRespMode>,
    /// 非 AI 流量（且 db 可用）的 body 累积缓冲，流结束时落库。
    body_buf: Option<String>,
    /// SSE 流量逐 chunk 落库开关（db 可用时开启；超过存储上限后关闭）。
    store_chunks: bool,
    /// 下一条落库 chunk 的序号（`response_chunks.seq`）。
    chunk_seq: i64,
    /// 已落库 chunk 累计字节数，超过 [`BODY_CAPTURE_LIMIT`] 后停止（DB 只存前缀）。
    chunk_bytes: usize,
    /// 距上次 AI 快照推送的累积字符数（节流用）。
    acc: usize,
    finalized: bool,
}

impl BodyObserver {
    fn new(ctx: &ProxyCtx, is_sse: bool) -> Self {
        let ai_req = ctx.ai_req();
        let ai_mode = ai_req.as_ref().map(|(provider, _, _)| {
            if is_sse {
                AiRespMode::Streaming(super::ai::stream::StreamParser::new(*provider))
            } else {
                AiRespMode::NonStreaming(String::new())
            }
        });
        let db = ctx.db_ref().cloned();
        let db_id = ctx.db_id();
        let needs_body_buf = db.is_some() && db_id.is_some() && ai_mode.is_none() && !is_sse;
        let store_chunks = db.is_some() && db_id.is_some() && is_sse;
        Self {
            request_id: ctx.request_id().to_string(),
            sender: ctx.sender().clone(),
            ai_req,
            sessions: ctx.sessions().cloned(),
            db,
            db_id,
            ai_mode,
            body_buf: needs_body_buf.then(String::new),
            store_chunks,
            chunk_seq: 0,
            chunk_bytes: 0,
            acc: 0,
            finalized: false,
        }
    }

    /// 每 chunk 观测：累积 body、AI 旁挂增量解析、推送原始文本（不改透传字节）。
    fn on_chunk(&mut self, bytes: &Bytes) {
        let chunk_str = String::from_utf8_lossy(bytes).into_owned();
        info!("Response chunk: {chunk_str}");
        // ── SSE 逐 chunk 落库（writer 线程异步写，此处仅发消息）──
        if self.store_chunks {
            if let (Some(db), Some(db_id)) = (self.db.as_ref(), self.db_id) {
                db.insert_chunk(db_id, &chunk_str, self.chunk_seq, crate::utils::date::now_ms())
                    .ok();
                self.chunk_seq += 1;
                self.chunk_bytes += chunk_str.len();
                if self.chunk_bytes >= crate::utils::buf_pool::BODY_CAPTURE_LIMIT {
                    self.store_chunks = false;
                    log::warn!("[db] response_chunks capped for request {}", self.request_id);
                }
            }
        }
        // ── 非 AI 流量 body 累积（带上限）──
        if let Some(ref mut buf) = self.body_buf {
            push_capped(buf, &chunk_str);
        }
        // ── AI 旁挂（不改透传 result）──
        if let (Some(mode), Some((_, sid, _))) = (self.ai_mode.as_mut(), self.ai_req.as_ref()) {
            match mode {
                AiRespMode::Streaming(parser) => {
                    if parser.feed(bytes) {
                        self.acc += bytes.len();
                        if self.acc >= AI_EMIT_THRESHOLD {
                            self.acc = 0;
                            // 节流增量快照不重发 request_turns（请求侧已首发，
                            // 前端按 requestId 缓存），省去热路径深克隆 + 序列化。
                            emit_ai_normalized(
                                &self.sender,
                                &self.request_id,
                                sid,
                                Vec::new(),
                                parser.snapshot(),
                            );
                        }
                    }
                }
                AiRespMode::NonStreaming(buf) => {
                    push_capped(buf, &chunk_str);
                }
            }
        }
        // 原始文本流照发——放最后，chunk_str 直接 move 进事件，省一次全量拷贝
        if let Some(ref ch) = self.sender {
            let _ = ch.send(ProxyEvent::ResponseChunk {
                id: self.request_id.clone(),
                chunk: chunk_str,
            });
        }
    }

    /// 流结束（或客户端中途断开）定稿：DB 落库、AI 归一化定稿、提交会话 usage。
    /// 幂等：重复调用只生效一次。
    fn finalize(&mut self) {
        if self.finalized {
            return;
        }
        self.finalized = true;

        // ── DB 更新响应体（流结束时写入）──
        if let (Some(db), Some(db_id)) = (self.db.as_ref(), self.db_id) {
            match self.ai_mode.as_ref() {
                Some(AiRespMode::NonStreaming(buf)) => {
                    db.update_traffic_response_body(db_id, buf).ok();
                }
                // 流式（SSE）：body 巨大，不存完整 body。
                Some(AiRespMode::Streaming(_)) => {}
                // 非 AI 流量 → 用独立 body_buf。
                None => {
                    if let Some(ref body) = self.body_buf {
                        if !body.is_empty() {
                            db.update_traffic_response_body(db_id, body).ok();
                        }
                    }
                }
            }
        }

        // ── AI 定稿：推送末次快照，提交会话 usage ──
        if let (Some(mode), Some((provider, sid, req_turns))) =
            (self.ai_mode.as_mut(), self.ai_req.as_ref())
        {
            let conv = match mode {
                AiRespMode::Streaming(parser) => {
                    parser.finalize();
                    Some(parser.snapshot())
                }
                AiRespMode::NonStreaming(buf) => super::ai::parse_response_body(*provider, buf),
            };
            if let Some(conv) = conv {
                // 保持事件顺序（AiNormalized 先于 AiSession），此处每请求只克隆一次。
                // 定稿快照重新携带完整 request_turns：作为该请求的最终记录，
                // 前端中途挂载错过首发时也能在此自愈。
                emit_ai_normalized(
                    &self.sender,
                    &self.request_id,
                    sid,
                    req_turns.as_ref().clone(),
                    conv.clone(),
                );
                commit_ai_final(&self.sender, &self.sessions, &self.request_id, sid, &conv);
            }
        }
    }
}

fn log_body_chunks(body: Body, ctx: &ProxyCtx, is_sse: bool) -> Body {
    let observer = Arc::new(Mutex::new(BodyObserver::new(ctx, is_sse)));
    let drop_observer = observer.clone();

    let stream = rama::futures::stream::unfold(
        (body.into_data_stream(), observer),
        move |(mut ds, obs)| async move {
            match rama::futures::StreamExt::next(&mut ds).await {
                Some(result) => {
                    if let Ok(ref bytes) = result {
                        obs.lock().expect("body observer lock").on_chunk(bytes);
                    }
                    Some((result, (ds, obs)))
                }
                None => {
                    // 流正常结束：定稿。
                    obs.lock().expect("body observer lock").finalize();
                    None
                }
            }
        },
    );
    // 客户端中途断开时 body 会在未消费完前被 drop —— on_drop 兜底定稿，
    // 避免 AI 会话 usage / DB 响应体丢失（正常结束时 on_drop 自动解除，不会双触发）。
    Body::from_stream(stream).on_drop(move || {
        drop_observer.lock().expect("body observer lock").finalize();
    })
}

/// 推送一次 `AiNormalized` 快照。`request_turns` 与 `conv` 均按值 move 进事件。
/// 流式节流增量传空 Vec（请求侧首发与定稿快照才携带完整 turns，
/// 前端 useAiSessions 按 requestId 缓存），热路径零克隆。
fn emit_ai_normalized(
    sender: &Option<Channel<ProxyEvent>>,
    request_id: &str,
    session_id: &str,
    request_turns: Vec<super::ai::AiTurn>,
    conv: super::ai::AiConversation,
) {
    if let Some(ch) = sender {
        let _ = ch.send(ProxyEvent::AiNormalized {
            id: request_id.to_string(),
            session_id: session_id.to_string(),
            provider: conv.provider.clone(),
            request_turns,
            streaming: conv.streaming,
            conversation: conv,
        });
    }
}

/// 响应定稿提交：写入会话标题（仅首请求）、累加 usage，并推送更新后的 `AiSession`。
fn commit_ai_final(
    sender: &Option<Channel<ProxyEvent>>,
    sessions: &Option<std::sync::Arc<std::sync::Mutex<super::ai::session::SessionStore>>>,
    request_id: &str,
    session_id: &str,
    conv: &super::ai::AiConversation,
) {
    let Some(sessions) = sessions else {
        return;
    };
    let mut store = sessions.lock().expect("sessions lock");
    store.set_title_if_first(session_id, request_id, conv);
    if let Some(usage) = conv.usage.as_ref() {
        store.add_usage(session_id, usage);
    }
    if let Some(entry) = store.get(session_id) {
        if let Some(ch) = sender {
            let _ = ch.send(ProxyEvent::AiSession {
                session_id: session_id.to_string(),
                scope_host: entry.scope.1.clone(),
                request_ids: entry.request_ids.clone(),
                usage_total: entry.usage_total.clone(),
                turn_count: entry.last_fingerprints.len() as u32,
                match_reason: "usage".to_string(),
                title: entry.title.clone(),
                source: entry.source.clone(),
            });
        }
    }
}

/// Build a generic error response returned directly to the client.
pub(crate) fn error_response(status: StatusCode, body: impl Into<Body>) -> Response {
    Response::builder()
        .status(status)
        .body(body.into())
        .expect("valid status code and body for error response")
}

/// 会话合并 header 尝试名单：规则各来源的 merge_header 在前（保序、去空白、
/// 大小写去重，附带来源名供命中归属），全局名单在后（无来源名，对前者去重）。
/// 来源名空白时该头仍参与分组，只是不产生归属。
fn session_header_list(
    sources: &[AiRuleSource],
    global: &[String],
) -> Vec<(String, Option<String>)> {
    let mut list: Vec<(String, Option<String>)> = Vec::with_capacity(sources.len() + global.len());
    for s in sources {
        let header = s.merge_header.trim();
        if header.is_empty() || list.iter().any(|(h, _)| h.eq_ignore_ascii_case(header)) {
            continue;
        }
        let name = s.name.trim();
        list.push((
            header.to_string(),
            (!name.is_empty()).then(|| name.to_string()),
        ));
    }
    for g in global {
        if !list.iter().any(|(h, _)| h.eq_ignore_ascii_case(g)) {
            list.push((g.clone(), None));
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::session_header_list;
    use crate::config::AiRuleSource;

    fn src(name: &str, header: &str) -> AiRuleSource {
        AiRuleSource {
            name: name.into(),
            merge_header: header.into(),
        }
    }

    #[test]
    fn source_headers_prepend_before_global() {
        let sources = vec![
            src("Claude Code", "x-claude-code-session-id"),
            src("Cursor", "x-cursor-session"),
        ];
        let global = vec!["x-session-id".to_string()];
        let list = session_header_list(&sources, &global);
        assert_eq!(
            list,
            vec![
                (
                    "x-claude-code-session-id".to_string(),
                    Some("Claude Code".to_string())
                ),
                ("x-cursor-session".to_string(), Some("Cursor".to_string())),
                ("x-session-id".to_string(), None),
            ]
        );
    }

    #[test]
    fn empty_sources_return_global_only() {
        let global = vec!["x-session-id".to_string()];
        assert_eq!(
            session_header_list(&[], &global),
            vec![("x-session-id".to_string(), None)]
        );
    }

    #[test]
    fn dedups_global_header_already_in_sources() {
        let sources = vec![src("Cursor", "X-Session-Id")];
        let global = vec!["x-session-id".to_string(), "x-other".to_string()];
        let list = session_header_list(&sources, &global);
        assert_eq!(
            list,
            vec![
                ("X-Session-Id".to_string(), Some("Cursor".to_string())),
                ("x-other".to_string(), None),
            ]
        );
    }

    #[test]
    fn blank_or_duplicate_source_headers_skipped() {
        let sources = vec![src("A", "   "), src("B", "x-h"), src("C", "X-H")];
        let list = session_header_list(&sources, &[]);
        // 空白头跳过；同名头（不区分大小写）首个生效
        assert_eq!(list, vec![("x-h".to_string(), Some("B".to_string()))]);
    }

    #[test]
    fn blank_source_name_keeps_header_without_attribution() {
        let sources = vec![src("  ", "x-h")];
        let list = session_header_list(&sources, &[]);
        assert_eq!(list, vec![("x-h".to_string(), None)]);
    }
}
