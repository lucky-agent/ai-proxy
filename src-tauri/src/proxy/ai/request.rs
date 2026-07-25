use crate::config::AiRuleSource;
use crate::proxy::ctx::ProxyCtx;
use crate::proxy::events::ProxyEvent;

use super::Provider;

/// 请求侧 AI 管线入口：provider 判定 → 请求归一化 → 会话分组 → 前端推送。
/// `body_str` 由调用方 clone 传入，内部消费，不产生额外分配。
pub(crate) fn process_ai_request(ctx: &ProxyCtx, body_str: Option<String>) {
    // AI 检测总开关关闭 / body 为空 → 完全跳过。
    let Some(body_str) = body_str else {
        return;
    };
    if !ctx.settings().ai.enabled {
        return;
    }
    let host = ctx.host_str();

    // 非 AI 流量（未命中 URL 规则）直接跳过，零开销。
    let (ai_hint, sources) = ctx
        .settings()
        .ai
        .detection
        .compute_hint(&host, &ctx.uri().path_or_root());
    let Some(provider) = ai_hint.map(Provider::from) else {
        return;
    };
    let Some(sessions) = ctx.sessions() else {
        return;
    };

    let root: serde_json::Value = match serde_json::from_str(&body_str) {
        Ok(v) => v,
        Err(_) => return,
    };
    let turns = provider.parse_request(&root);
    if turns.is_empty() {
        return;
    }

    let cfg = &ctx.settings().ai.session;

    // 分组 + 会话快照一次锁内完成（AssignResult 自带快照，免二次加锁读表）。
    let session_headers = session_header_list(&sources, &cfg.session_headers);
    let result = {
        let mut store = sessions.lock().expect("sessions lock");
        store.assign(super::session::AssignParams {
            provider,
            host: &host,
            session_headers: &session_headers,
            headers: ctx.header_map(),
            messages: &turns,
            prefix_fallback: cfg.prefix_match_fallback,
            request_id: ctx.request_id(),
        })
    };

    // 登记 (provider, session_id) 供响应侧使用。
    ctx.set_ai_req(provider, result.session_id.clone());

    // 请求侧 turns 存入 ctx，供响应侧构造自包含的 AiNormalized。
    ctx.set_ai_request_turns(turns);

    ctx.send(ProxyEvent::AiSession {
        session_id: result.session_id,
        scope_host: host,
        request_ids: result.request_ids,
        usage_total: result.usage_total,
        match_reason: result.match_reason,
        title: result.title,
        source: result.source,
    });
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
