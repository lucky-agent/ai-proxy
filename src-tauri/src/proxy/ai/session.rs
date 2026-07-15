//! 跨请求会话分组。
//!
//! 归一化的副产品：请求归一化时 messages 与 headers 都在手，就地判定会话归属。
//! - 范围（scope）= provider + 上游 host，只在同一 scope 内分组；
//! - 会话区分：① 配置的 session header 值优先 → ② 消息前缀匹配兜底 → ③ 新会话；
//! - token 简单累加；会话表内存 + LRU 上限；不持久化。

use std::collections::HashMap;

use uuid::Uuid;

use super::normalize::{AiTurn, AiUsage};
use super::Provider;

/// 一个会话的内存状态。
pub(crate) struct SessionEntry {
    pub id: String,
    pub scope: (String, String),
    pub request_ids: Vec<String>,
    /// 供前缀匹配用：该会话最近一次请求的完整 messages（不含响应 turn）。
    pub last_messages: Vec<AiTurn>,
    pub usage_total: AiUsage,
    /// LRU 序号，越大越新。
    pub last_touched: u64,
}

/// 分组结果。
pub(crate) struct AssignResult {
    pub session_id: String,
    /// 归组依据：`header:<name>` / `prefix` / `new`。
    pub match_reason: String,
}

/// 会话状态表。挂在 `State` 上，`Arc<Mutex<..>>` 包裹以线程安全。
pub(crate) struct SessionStore {
    sessions: HashMap<String, SessionEntry>,
    max_sessions: usize,
    tick: u64,
}

impl SessionStore {
    pub(crate) fn new(max_sessions: usize) -> Self {
        SessionStore {
            sessions: HashMap::new(),
            max_sessions: max_sessions.max(1),
            tick: 0,
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// 判定请求归属并登记。返回会话 id 与归组依据。
    ///
    /// - `session_headers`：配置的 header 名单，按顺序取第一个命中；
    /// - `headers`：本次请求头（小写键）；
    /// - `messages`：本次请求归一化后的 turns（用于前缀匹配与更新 last_messages）；
    /// - `prefix_fallback`：无 header 时是否启用前缀匹配。
    pub(crate) fn assign(
        &mut self,
        provider: Provider,
        host: &str,
        session_headers: &[String],
        headers: &HashMap<String, String>,
        messages: &[AiTurn],
        prefix_fallback: bool,
        request_id: &str,
    ) -> AssignResult {
        let scope = (provider.as_str().to_string(), host.to_string());
        let tick = self.next_tick();

        // ① header 优先：按名单顺序找第一个命中，会话 id = scope + header 值
        for name in session_headers {
            if let Some(val) = lookup_header(headers, name) {
                let sid = session_key(&scope, val);
                self.touch_or_create(&sid, &scope, messages, request_id, tick);
                return AssignResult {
                    session_id: sid,
                    match_reason: format!("header:{name}"),
                };
            }
        }

        // ② 前缀匹配兜底：同 scope 会话里找 last_messages 是本次前缀者，取最长
        if prefix_fallback {
            let mut best: Option<(String, usize)> = None;
            for entry in self.sessions.values() {
                if entry.scope != scope {
                    continue;
                }
                if is_prefix(&entry.last_messages, messages) {
                    let len = entry.last_messages.len();
                    if best.as_ref().map(|(_, l)| len > *l).unwrap_or(true) {
                        best = Some((entry.id.clone(), len));
                    }
                }
            }
            if let Some((sid, _)) = best {
                self.touch_or_create(&sid, &scope, messages, request_id, tick);
                return AssignResult {
                    session_id: sid,
                    match_reason: "prefix".to_string(),
                };
            }
        }

        // ③ 新会话
        let sid = format!("sess-{}", Uuid::new_v4());
        self.touch_or_create(&sid, &scope, messages, request_id, tick);
        AssignResult {
            session_id: sid,
            match_reason: "new".to_string(),
        }
    }

    fn touch_or_create(
        &mut self,
        sid: &str,
        scope: &(String, String),
        messages: &[AiTurn],
        request_id: &str,
        tick: u64,
    ) {
        match self.sessions.get_mut(sid) {
            Some(entry) => {
                if !entry.request_ids.iter().any(|r| r == request_id) {
                    entry.request_ids.push(request_id.to_string());
                }
                entry.last_messages = messages.to_vec();
                entry.last_touched = tick;
            }
            None => {
                self.sessions.insert(
                    sid.to_string(),
                    SessionEntry {
                        id: sid.to_string(),
                        scope: scope.clone(),
                        request_ids: vec![request_id.to_string()],
                        last_messages: messages.to_vec(),
                        usage_total: AiUsage::default(),
                        last_touched: tick,
                    },
                );
                self.evict_if_needed();
            }
        }
    }

    /// 把某请求的 usage 累加到其所属会话。
    pub(crate) fn add_usage(&mut self, session_id: &str, usage: &AiUsage) {
        if let Some(entry) = self.sessions.get_mut(session_id) {
            entry.usage_total.accumulate(usage);
        }
    }

    /// 读取会话快照（供构造 AiSession 事件）。
    pub(crate) fn get(&self, session_id: &str) -> Option<&SessionEntry> {
        self.sessions.get(session_id)
    }

    /// 超出上限时按 LRU（last_touched 最小）淘汰。
    fn evict_if_needed(&mut self) {
        while self.sessions.len() > self.max_sessions {
            let Some(victim) = self
                .sessions
                .values()
                .min_by_key(|e| e.last_touched)
                .map(|e| e.id.clone())
            else {
                break;
            };
            self.sessions.remove(&victim);
            log::info!("[ai-session] evicted LRU session {victim}");
        }
    }
}

/// 大小写不敏感查找请求头。
fn lookup_header<'a>(headers: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    let lname = name.to_ascii_lowercase();
    headers
        .iter()
        .find(|(k, _)| k.to_ascii_lowercase() == lname)
        .map(|(_, v)| v.as_str())
}

fn session_key(scope: &(String, String), header_val: &str) -> String {
    format!("{}|{}|{}", scope.0, scope.1, header_val)
}

/// `prev` 是否为 `curr` 的前缀：逐 turn 比较 role 与 content 文本。
pub(crate) fn is_prefix(prev: &[AiTurn], curr: &[AiTurn]) -> bool {
    if prev.is_empty() || prev.len() > curr.len() {
        return false;
    }
    prev.iter().zip(curr.iter()).all(|(a, b)| a == b)
}
