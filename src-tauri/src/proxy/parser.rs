use std::collections::HashMap;

use log::info;
use rama::http::{Body, Response, StatusCode, request};
use tauri::ipc::Channel;

use crate::proxy::state::ProxyCtx;

use super::events::ProxyEvent;

/// 简易 URL 百分号解码
fn url_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            // 无效的百分号编码，保留原样
            result.push('%');
            result.push_str(&hex);
        } else if ch == '+' {
            result.push(' ');
        } else {
            result.push(ch);
        }
    }
    result
}

/// Accepts body as raw bytes to avoid unnecessary UTF-8 allocation;
/// only converts lossily when emitting the RequestChunk event.
pub(crate) fn log_request(ctx: &ProxyCtx, parts: &rama::http::request::Parts, body: &[u8]) {
    let query_params: HashMap<String, String> = ctx
        .uri()
        .query()
        .map(|q| {
            q.as_encoded_str()
                .split('&')
                .filter_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    let key = parts.next()?.to_string();
                    let value = parts.next().unwrap_or("").to_string();
                    let decoded_key = url_decode(&key);
                    let decoded_value = url_decode(&value);
                    Some((decoded_key, decoded_value))
                })
                .collect()
        })
        .unwrap_or_default();

    let req_headers: HashMap<String, String> = parts
        .headers
        .iter()
        .filter_map(|(name, value)| {
            let key = name.to_string();
            let val = value.to_str().ok()?.to_string();
            // 合并重复的 header（如 Cookie 可能在请求头中出现多次）
            Some((key, val))
        })
        .fold(HashMap::new(), |mut acc, (key, val)| {
            if let Some(existing) = acc.get_mut(&key) {
                existing.push_str("; ");
                existing.push_str(&val);
            } else {
                acc.insert(key, val);
            }
            acc
        });

    let req_content_type = parts
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let req_content_length = parts
        .headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let host_hint = parts
        .headers
        .get("host")
        .and_then(|v| v.to_str().ok());

    let ai_hint = crate::proxy::ai_hint::compute_ai_hint(ctx.uri(), host_hint, ctx.settings());

    ctx.send(ProxyEvent::Request {
        id: ctx.request_id().to_string(),
        method: ctx.method().to_string(),
        uri: ctx.uri().to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        headers: req_headers.clone(),
        query_params,
        decrypted: true,
        content_type: req_content_type,
        content_length: req_content_length,
        ai_hint: ai_hint.clone(),
    });
    if !body.is_empty() {
        ctx.send(ProxyEvent::RequestChunk {
            id: ctx.request_id().to_string(),
            chunk: String::from_utf8_lossy(body).into_owned(),
        });
    }

    // ── AI 会话分组（归一化的副产品）──
    log_ai_request(ctx, &ai_hint, &req_headers, host_hint, body);
}

/// 请求侧 AI 归一化 + 会话分组。仅 AI 流量执行；判定结果存 ctx 供响应侧消费。
fn log_ai_request(
    ctx: &ProxyCtx,
    ai_hint: &super::events::AiHint,
    req_headers: &HashMap<String, String>,
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

    let result = {
        log::info!("[probe] {} sessions.lock(assign) acquiring", ctx.request_id());
        let mut store = sessions.lock().expect("sessions lock");
        store.assign(
            provider,
            &host,
            &cfg.session_headers,
            req_headers,
            &turns,
            cfg.prefix_match_fallback,
            ctx.request_id(),
        )
    };
    log::info!("[probe] {} sessions.lock(assign) released", ctx.request_id());
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
    emit_ai_normalized(
        ctx.sender(),
        ctx.request_id(),
        &result.session_id,
        &turns,
        &req_snapshot,
    );

    ctx.set_ai_req(provider, result.session_id.clone(), turns);
    emit_ai_session(ctx, &result.session_id, &host, &result.match_reason);
}

/// 从会话表读取快照并推送 `AiSession` 事件。
fn emit_ai_session(ctx: &ProxyCtx, session_id: &str, host: &str, match_reason: &str) {
    let Some(sessions) = ctx.sessions() else {
        return;
    };
    log::info!("[probe] {} sessions.lock(emit) acquiring", ctx.request_id());
    let store = sessions.lock().expect("sessions lock");
    if let Some(entry) = store.get(session_id) {
        ctx.send(ProxyEvent::AiSession {
            session_id: session_id.to_string(),
            scope_host: host.to_string(),
            request_ids: entry.request_ids.clone(),
            usage_total: entry.usage_total.clone(),
            turn_count: entry.request_ids.len() as u32,
            match_reason: match_reason.to_string(),
        });
    }
    drop(store);
    log::info!("[probe] {} sessions.lock(emit) released", ctx.request_id());
}

/// Handle direct (non-tunnel) requests to the proxy itself.
/// Constructs a Response directly from the request parts and body.
pub(crate) fn direct_response(ctx: &ProxyCtx, req: request::Parts, body: Body) -> Response {
    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .body(body)
        .expect("valid status code and body for direct response");
    *resp.headers_mut() = req.headers.clone();
    log_response(ctx, resp)
}

/// Log response and emit via channel.
pub(crate) fn log_response(ctx: &ProxyCtx, resp: Response) -> Response {
    let status = resp.status();
    info!("Response [{} {}] {}", ctx.method(), ctx.uri(), status);

    let (parts, body) = resp.into_parts();

    let resp_headers: HashMap<String, String> = parts
        .headers
        .iter()
        .filter_map(|(name, value)| {
            let key = name.to_string();
            let val = value.to_str().ok()?.to_string();
            Some((key, val))
        })
        .fold(HashMap::new(), |mut acc, (key, val)| {
            // Set-Cookie 可能多次出现，合并为新行分隔
            if key.to_lowercase() == "set-cookie" {
                if let Some(existing) = acc.get_mut(&key) {
                    existing.push('\n');
                    existing.push_str(&val);
                } else {
                    acc.insert(key, val);
                }
            } else if let Some(existing) = acc.get_mut(&key) {
                existing.push_str(", ");
                existing.push_str(&val);
            } else {
                acc.insert(key, val);
            }
            acc
        });

    let resp_content_type = parts
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let resp_content_length = parts
        .headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let is_sse = resp_content_type
        .as_deref()
        .map(|ct| ct.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false);

    ctx.send(ProxyEvent::Response {
        id: ctx.request_id().to_string(),
        status: status.as_u16(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        duration_ms: ctx.duration_ms(),
        headers: resp_headers,
        content_type: resp_content_type,
        content_length: resp_content_length,
    });

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

fn log_body_chunks(body: Body, ctx: &ProxyCtx, is_sse: bool) -> Body {
    let request_id = ctx.request_id().to_string();
    let sender = ctx.sender().clone();

    // 请求侧判定的 (provider, session_id, request_turns)；None 表示非 AI 流量，走原路径零开销。
    let ai_req = ctx.ai_req();
    let sessions = ctx.sessions().cloned();
    let ai_mode = ai_req.as_ref().map(|(provider, _, _)| {
        if is_sse {
            AiRespMode::Streaming(super::ai::stream::StreamParser::new(*provider))
        } else {
            AiRespMode::NonStreaming(String::new())
        }
    });

    // unfold 状态：(数据流, AI 模式, 距上次推送累积字符数)
    let init = (body.into_data_stream(), ai_mode, 0usize);
    let stream = rama::futures::stream::unfold(init, move |(mut ds, mut ai_mode, mut acc)| {
        let request_id = request_id.clone();
        let sender = sender.clone();
        let ai_req = ai_req.clone();
        let sessions = sessions.clone();
        async move {
            match rama::futures::StreamExt::next(&mut ds).await {
                Some(result) => {
                    if let Ok(ref bytes) = result {
                        let chunk_str = String::from_utf8_lossy(bytes).into_owned();
                        info!("Response chunk: {chunk_str}");
                        // 原始文本流照发（透传字节不变）
                        if let Some(ref ch) = sender {
                            let _ = ch.send(ProxyEvent::ResponseChunk {
                                id: request_id.clone(),
                                chunk: chunk_str.clone(),
                            });
                        }
                        // ── AI 旁挂（不改透传 result）──
                        if let (Some(mode), Some((_, sid, req_turns))) = (ai_mode.as_mut(), ai_req.as_ref()) {
                            match mode {
                                AiRespMode::Streaming(parser) => {
                                    if parser.feed(bytes) {
                                        acc += bytes.len();
                                        if acc >= AI_EMIT_THRESHOLD {
                                            acc = 0;
                                            emit_ai_normalized(
                                                &sender,
                                                &request_id,
                                                sid,
                                                req_turns,
                                                &parser.snapshot(),
                                            );
                                        }
                                    }
                                }
                                AiRespMode::NonStreaming(buf) => {
                                    buf.push_str(&chunk_str);
                                }
                            }
                        }
                    }
                    Some((result, (ds, ai_mode, acc)))
                }
                None => {
                    // 流结束：定稿归一化，推送末次快照，提交会话 usage。
                    if let (Some(mode), Some((provider, sid, req_turns))) =
                        (ai_mode.as_mut(), ai_req.as_ref())
                    {
                        let conv = match mode {
                            AiRespMode::Streaming(parser) => {
                                parser.finalize();
                                Some(parser.snapshot())
                            }
                            AiRespMode::NonStreaming(buf) => {
                                super::ai::parse_response_body(*provider, buf)
                            }
                        };
                        if let Some(conv) = conv {
                            emit_ai_normalized(&sender, &request_id, sid, req_turns, &conv);
                            commit_ai_usage(&sender, &sessions, sid, &conv);
                        }
                    }
                    None
                }
            }
        }
    });
    Body::from_stream(stream)
}

/// 推送一次 `AiNormalized` 快照。
fn emit_ai_normalized(
    sender: &Option<Channel<ProxyEvent>>,
    request_id: &str,
    session_id: &str,
    request_turns: &[super::ai::AiTurn],
    conv: &super::ai::AiConversation,
) {
    if let Some(ch) = sender {
        let _ = ch.send(ProxyEvent::AiNormalized {
            id: request_id.to_string(),
            session_id: session_id.to_string(),
            provider: conv.provider.clone(),
            request_turns: request_turns.to_vec(),
            conversation: conv.clone(),
            streaming: conv.streaming,
        });
    }
}

/// 把响应 usage 累加进会话表并推送更新后的 `AiSession`。
fn commit_ai_usage(
    sender: &Option<Channel<ProxyEvent>>,
    sessions: &Option<std::sync::Arc<std::sync::Mutex<super::ai::session::SessionStore>>>,
    session_id: &str,
    conv: &super::ai::AiConversation,
) {
    let (Some(sessions), Some(usage)) = (sessions, conv.usage.as_ref()) else {
        return;
    };
    log::info!("[probe] sessions.lock(usage) acquiring sid={session_id}");
    let mut store = sessions.lock().expect("sessions lock");
    store.add_usage(session_id, usage);
    if let Some(entry) = store.get(session_id) {
        if let Some(ch) = sender {
            let _ = ch.send(ProxyEvent::AiSession {
                session_id: session_id.to_string(),
                scope_host: entry.scope.1.clone(),
                request_ids: entry.request_ids.clone(),
                usage_total: entry.usage_total.clone(),
                turn_count: entry.last_messages.len() as u32,
                match_reason: "usage".to_string(),
            });
        }
    }
    drop(store);
    log::info!("[probe] sessions.lock(usage) released sid={session_id}");
}

/// Build a generic error response returned directly to the client.
pub(crate) fn error_response(status: StatusCode, body: impl Into<Body>) -> Response {
    Response::builder()
        .status(status)
        .body(body.into())
        .expect("valid status code and body for error response")
}
