//! 跨请求会话分组。
//!
//! 归一化的副产品：请求归一化时 messages 与 headers 都在手，就地判定会话归属。
//! - 范围（scope）= provider + 上游 host，只在同一 scope 内分组；
//! - 会话区分：① 配置的 session header 值优先 → ② 消息前缀匹配兜底 → ③ 新会话；
//! - token 简单累加；会话表内存 + LRU 上限；不持久化。

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use uuid::Uuid;

use super::Provider;
use super::normalize::{AiConversation, AiTurn, AiUsage};

/// 一个会话的内存状态。
pub(crate) struct SessionEntry {
    pub id: String,
    pub scope: (String, String),
    pub request_ids: Vec<u64>,
    /// 供前缀匹配用：该会话最近一次请求各 turn 的指纹链（不含响应 turn）。
    /// 前缀匹配只需相等性判定，无需原文——每 turn 一个哈希，
    /// 内存 O(轮次) 而非 O(内容)，比较为 u64 切片比较。
    pub last_fingerprints: Vec<u64>,
    pub usage_total: AiUsage,
    /// 会话标题：来自首请求响应的 `{"title": "..."}`（见 normalize::extract_title）。
    pub title: Option<String>,
    /// 来源归属：规则内 (来源, 合并头) 对的头命中时写入对应来源名；
    /// 全局名单命中或前缀/新会话为 None。前缀续轮不清除已有归属。
    pub source: Option<String>,
    /// 归组依据：`header:<name>` / `prefix` / `new`。随每次 assign 更新，
    /// 供 usage 更新事件回传，避免覆盖为 "usage"。
    pub match_reason: String,
    /// LRU 序号，越大越新。
    pub last_touched: u64,
}

/// 分组结果 + 会话快照。快照与分组在同一次锁内读取，
/// 调用方构造 `AiSession` 事件无需二次加锁。
pub(crate) struct AssignResult {
    pub session_id: String,
    /// 归组依据：`header:<name>` / `prefix` / `new`。
    pub match_reason: String,
    pub request_ids: Vec<u64>,
    pub usage_total: AiUsage,
    pub title: Option<String>,
    /// 会话来源归属（见 [`SessionEntry::source`]）。
    pub source: Option<String>,
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

    /// 判定请求归属并登记。返回会话 id、归组依据与会话快照。
    ///
    /// - `session_headers`：(header, 该头所属来源名) 尝试名单（规则来源对在前、
    ///   全局名单在后，见 parser::session_header_list），按顺序取第一个命中；
    ///   命中带来源名的头即把会话归属该来源；
    /// - `headers`：本次请求头（键为小写，来自 rama `HeaderName`）；
    /// - `messages`：本次请求归一化后的 turns（用于前缀匹配与更新指纹链）；
    /// - `prefix_fallback`：无 header 时是否启用前缀匹配。
    pub(crate) fn assign(
        &mut self,
        provider: Provider,
        host: &str,
        session_headers: &[(String, Option<String>)],
        headers: &HashMap<String, String>,
        messages: &[AiTurn],
        prefix_fallback: bool,
        request_id: u64,
    ) -> AssignResult {
        let scope = (provider.as_str().to_string(), host.to_string());
        let tick = self.next_tick();
        let fingerprints: Vec<u64> = messages.iter().map(turn_fingerprint).collect();

        // ① header 优先：按名单顺序取第一个命中，会话 id = scope + header 值
        for (name, source) in session_headers {
            if let Some(val) = headers.get(&name.to_ascii_lowercase()) {
                let sid = session_key(&scope, val);
                let reason = format!("header:{name}");
                self.touch_or_create(
                    &sid,
                    &scope,
                    fingerprints.clone(),
                    request_id,
                    tick,
                    source.as_deref(),
                    &reason,
                    messages,
                );
                return self.result(sid, reason);
            }
        }

        // ② 前缀匹配兜底：同 scope 会话里找指纹链是本次前缀者，取最长
        if prefix_fallback {
            let mut best: Option<(String, usize)> = None;
            for entry in self.sessions.values() {
                if entry.scope != scope {
                    continue;
                }
                if is_prefix(&entry.last_fingerprints, &fingerprints) {
                    let len = entry.last_fingerprints.len();
                    if best.as_ref().map(|(_, l)| len > *l).unwrap_or(true) {
                        best = Some((entry.id.clone(), len));
                    }
                }
            }
            if let Some((sid, _)) = best {
                self.touch_or_create(
                    &sid,
                    &scope,
                    fingerprints,
                    request_id,
                    tick,
                    None,
                    "prefix",
                    messages,
                );
                return self.result(sid, "prefix".to_string());
            }
        }

        // ③ 新会话
        let sid = format!("sess-{}", Uuid::new_v4());
        self.touch_or_create(
            &sid,
            &scope,
            fingerprints,
            request_id,
            tick,
            None,
            "new",
            messages,
        );
        self.result(sid, "new".to_string())
    }

    /// 分组落定后就地读快照。新会话 tick 最大不会被 LRU 淘汰，entry 必然存在。
    fn result(&self, session_id: String, match_reason: String) -> AssignResult {
        let entry = &self.sessions[&session_id];
        AssignResult {
            request_ids: entry.request_ids.clone(),
            usage_total: entry.usage_total.clone(),
            title: entry.title.clone(),
            source: entry.source.clone(),
            session_id,
            match_reason,
        }
    }

    fn touch_or_create(
        &mut self,
        sid: &str,
        scope: &(String, String),
        fingerprints: Vec<u64>,
        request_id: u64,
        tick: u64,
        source: Option<&str>,
        match_reason: &str,
        messages: &[AiTurn],
    ) {
        match self.sessions.get_mut(sid) {
            Some(entry) => {
                if !entry.request_ids.iter().any(|&r| r == request_id) {
                    entry.request_ids.push(request_id);
                }
                entry.last_fingerprints = fingerprints;
                entry.last_touched = tick;
                entry.match_reason = match_reason.to_string();
                // 仅在本次确认了来源时覆写；前缀/全局命中（None）不清除已有归属
                if let Some(src) = source {
                    entry.source = Some(src.to_string());
                }
            }
            None => {
                // 仅新会话时从第一条 user turn 提取标题（兜底），
                // 后续由 refine_title 用响应 {"title": "..."} 覆盖
                let title = super::normalize::extract_title_from_request(messages);
                self.sessions.insert(
                    sid.to_string(),
                    SessionEntry {
                        id: sid.to_string(),
                        scope: scope.clone(),
                        request_ids: vec![request_id],
                        last_fingerprints: fingerprints,
                        usage_total: AiUsage::default(),
                        title,
                        source: source.map(str::to_string),
                        match_reason: match_reason.to_string(),
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

    /// 首请求响应定稿时，尝试用 `{"title": "..."}`（模型生成标题）
    /// 覆盖 `assign` 阶段从用户消息提取的兜底标题。
    /// 非首请求的响应不操作（会话标题由首次交互定义）。
    pub(crate) fn refine_title(
        &mut self,
        session_id: &str,
        request_id: u64,
        conv: &AiConversation,
    ) {
        let Some(entry) = self.sessions.get_mut(session_id) else {
            return;
        };
        // 仅首请求的响应参与标题命名
        if entry.request_ids.first().copied() != Some(request_id) {
            return;
        }
        // 响应有 {"title":"..."} → 覆盖（比用户原始输入更精炼）
        if let Some(title) = super::normalize::extract_title(conv) {
            entry.title = Some(title);
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

fn session_key(scope: &(String, String), header_val: &str) -> String {
    format!("{}|{}|{}", scope.0, scope.1, header_val)
}

/// 单 turn 指纹：role + content 序列化文本的哈希。
/// 同会话续轮时客户端原样重发历史，同一输入的序列化输出稳定，
/// 指纹相等即可代表 turn 相等（碰撞概率可忽略；仅内存态，不持久化）。
fn turn_fingerprint(turn: &AiTurn) -> u64 {
    let mut h = DefaultHasher::new();
    turn.role.hash(&mut h);
    if let Ok(json) = serde_json::to_string(&turn.content) {
        json.hash(&mut h);
    }
    h.finish()
}

/// `prev` 是否为 `curr` 的非空前缀（逐 turn 指纹比较）。
pub(crate) fn is_prefix(prev: &[u64], curr: &[u64]) -> bool {
    !prev.is_empty() && prev.len() <= curr.len() && prev == &curr[..prev.len()]
}
