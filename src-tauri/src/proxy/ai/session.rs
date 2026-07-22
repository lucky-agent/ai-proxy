//! 跨请求会话分组。
//!
//! 归一化的副产品：请求归一化时 messages 与 headers 都在手，就地判定会话归属。
//! - 范围（scope）= provider + 上游 host，只在同一 scope 内分组；
//! - 会话区分：① 配置的 session header 值优先 → ② 消息前缀匹配兜底 → ③ 新会话；
//! - token 简单累加；会话表内存 + LRU 上限；不持久化。

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Provider;
use super::normalize::{AiConversation, AiTurn, AiUsage};

/// 会话时间线中的一条记录。fingerprint 供 LCP 快速比较，turn 为完整内容。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimelineEntry {
    pub fingerprint: u64,
    pub turn: AiTurn,
    pub request_id: u64,
}

/// 一个会话的内存状态。
pub(crate) struct SessionEntry {
    pub id: String,
    pub scope: (String, String),
    pub request_ids: Vec<u64>,
    /// 供前缀匹配用：该会话最近一次请求各 turn 的指纹链（不含响应 turn）。
    /// 前缀匹配只需相等性判定，无需原文——每 turn 一个哈希，
    /// 内存 O(轮次) 而非 O(内容)，比较为 u64 切片比较。
    pub last_fingerprints: Vec<u64>,
    /// 已合并的时间线：所有请求 deltas 的累积，前端直接 append 即可渲染。
    pub timeline: Vec<TimelineEntry>,
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
    /// 本次请求体 turns 超出会话时间线的增量（LCP 之后的新增 turns）。
    /// 新会话时为全部请求 turns。
    pub request_delta: Vec<TimelineEntry>,
}

/// 后端内存统计（仅 SessionStore 维度——唯一全局持久的运行时缓存）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct BackendMemoryStats {
    pub session_count: usize,
    pub max_sessions: usize,
    /// 所有 session 的 timeline 条目总数。
    pub timeline_entry_count: usize,
    /// timeline 中 AiTurn 内容的 JSON 序列化字节估算。
    pub timeline_content_bytes: u64,
    /// 字符串元数据：id / scope / title / source / match_reason 字节估算（UTF-8 len）。
    pub metadata_bytes: u64,
    /// last_fingerprints + request_ids 两个 Vec<u64> 的字节估算。
    pub fingerprint_bytes: u64,
    /// HashMap 条目 + Vec 堆分配 + SessionEntry 固定字段的结构开销。
    pub struct_bytes: u64,
    pub total_est_bytes: u64,
}

impl Default for BackendMemoryStats {
    fn default() -> Self {
        Self {
            session_count: 0,
            max_sessions: 0,
            timeline_entry_count: 0,
            timeline_content_bytes: 0,
            metadata_bytes: 0,
            fingerprint_bytes: 0,
            struct_bytes: 0,
            total_est_bytes: 0,
        }
    }
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
    ///   全局名单在后，见 request::session_header_list），按顺序取第一个命中；
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
                let request_delta =
                    self.touch_or_create(&sid, &scope, &fingerprints, request_id, tick, source.as_deref(), &reason, messages);
                return self.result_with_delta(sid, reason, request_delta);
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
                let request_delta = self.touch_or_create(
                    &sid, &scope, &fingerprints, request_id, tick, None, "prefix", messages,
                );
                return self.result_with_delta(sid, "prefix".to_string(), request_delta);
            }
        }

        // ③ 新会话
        let sid = format!("sess-{}", Uuid::new_v4());
        let request_delta =
            self.touch_or_create(&sid, &scope, &fingerprints, request_id, tick, None, "new", messages);
        self.result_with_delta(sid, "new".to_string(), request_delta)
    }

    /// 分组落定后就地读快照（含 request_delta）。新会话 tick 最大不会被 LRU 淘汰，entry 必然存在。
    fn result_with_delta(
        &self,
        session_id: String,
        match_reason: String,
        request_delta: Vec<TimelineEntry>,
    ) -> AssignResult {
        let entry = &self.sessions[&session_id];
        AssignResult {
            request_ids: entry.request_ids.clone(),
            usage_total: entry.usage_total.clone(),
            title: entry.title.clone(),
            source: entry.source.clone(),
            session_id,
            match_reason,
            request_delta,
        }
    }

    /// 更新/创建会话条目，返回本次请求体 turns 超出时间线的增量。
    fn touch_or_create(
        &mut self,
        sid: &str,
        scope: &(String, String),
        fingerprints: &[u64],
        request_id: u64,
        tick: u64,
        source: Option<&str>,
        match_reason: &str,
        messages: &[AiTurn],
    ) -> Vec<TimelineEntry> {
        match self.sessions.get_mut(sid) {
            Some(entry) => {
                if !entry.request_ids.iter().any(|&r| r == request_id) {
                    entry.request_ids.push(request_id);
                }
                // 计算增量：LCP(timeline_fingerprints, new_fingerprints)
                let lcp = entry
                    .timeline
                    .iter()
                    .map(|e| e.fingerprint)
                    .zip(fingerprints.iter())
                    .take_while(|(a, b)| a == *b)
                    .count();
                let delta: Vec<TimelineEntry> = fingerprints[lcp..]
                    .iter()
                    .zip(&messages[lcp..])
                    .map(|(fp, turn)| TimelineEntry {
                        fingerprint: *fp,
                        turn: turn.clone(),
                        request_id,
                    })
                    .collect();
                // 追加到时间线
                entry.timeline.extend(delta.clone());
                entry.last_fingerprints = fingerprints.to_vec();
                entry.last_touched = tick;
                entry.match_reason = match_reason.to_string();
                // 仅在本次确认了来源时覆写；前缀/全局命中（None）不清除已有归属
                if let Some(src) = source {
                    entry.source = Some(src.to_string());
                }
                delta
            }
            None => {
                // 新会话：全部 turns 都是增量
                let delta: Vec<TimelineEntry> = messages
                    .iter()
                    .zip(fingerprints.iter())
                    .map(|(turn, fp)| TimelineEntry {
                        fingerprint: *fp,
                        turn: turn.clone(),
                        request_id,
                    })
                    .collect();
                // 仅新会话时从第一条 user turn 提取标题（兜底），
                // 后续由 refine_title 用响应 {"title": "..."} 覆盖
                let title = super::normalize::extract_title_from_request(messages);
                self.sessions.insert(
                    sid.to_string(),
                    SessionEntry {
                        id: sid.to_string(),
                        scope: scope.clone(),
                        request_ids: vec![request_id],
                        last_fingerprints: fingerprints.to_vec(),
                        timeline: delta.clone(),
                        usage_total: AiUsage::default(),
                        title,
                        source: source.map(str::to_string),
                        match_reason: match_reason.to_string(),
                        last_touched: tick,
                    },
                );
                self.evict_if_needed();
                delta
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

    /// 将 assistant turn 追加到会话时间线（响应定稿时调用）。
    /// 返回新增的 timeline entries 供前端 AiTimelineDelta 事件。
    pub(crate) fn append_assistant_turns(
        &mut self,
        session_id: &str,
        request_id: u64,
        turns: &[AiTurn],
    ) -> Vec<TimelineEntry> {
        let Some(entry) = self.sessions.get_mut(session_id) else {
            return Vec::new();
        };
        let entries: Vec<TimelineEntry> = turns
            .iter()
            .map(|turn| TimelineEntry {
                fingerprint: turn_fingerprint(turn),
                turn: turn.clone(),
                request_id,
            })
            .collect();
        entry.timeline.extend(entries.clone());
        entries
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

    /// Walk all sessions to produce a backend-memory snapshot.
    /// This is the only globally-persistent runtime cache (per-request buffers are freed on drop).
    pub(crate) fn memory_stats(&self) -> BackendMemoryStats {
        let mut timeline_entry_count: usize = 0;
        let mut timeline_content_bytes: u64 = 0;
        let mut metadata_bytes: u64 = 0;

        for entry in self.sessions.values() {
            timeline_entry_count += entry.timeline.len();
            // Timeline content: JSON-serialised AiTurn per entry.
            for te in &entry.timeline {
                if let Ok(json) = serde_json::to_string(&te.turn) {
                    timeline_content_bytes += json.len() as u64;
                }
            }
            // String metadata (UTF-8 length ≈ byte footprint of the allocation).
            metadata_bytes += entry.id.len() as u64;
            metadata_bytes += entry.scope.0.len() as u64;
            metadata_bytes += entry.scope.1.len() as u64;
            metadata_bytes += entry.title.as_deref().map_or(0, |s| s.len()) as u64;
            metadata_bytes += entry.source.as_deref().map_or(0, |s| s.len()) as u64;
            metadata_bytes += entry.match_reason.len() as u64;
        }

        let session_count = self.sessions.len();

        // Fingerprint + request_ids: two Vec<u64> per session, count the heap storage.
        let fingerprint_bytes: u64 = self
            .sessions
            .values()
            .map(|e| {
                ((e.last_fingerprints.len() + e.request_ids.len())
                    * std::mem::size_of::<u64>()) as u64
            })
            .sum();

        // Structural overhead (≈ Rust allocator overhead for the HashMap + Vecs + Strings).
        const HASHMAP_BUCKET_COST: u64 = 32;
        const ENTRY_BASE_COST: u64 = 128;
        const VEC_HEAP_PER_SLOT: usize = 8;

        let mut struct_bytes: u64 = 0;
        struct_bytes += session_count as u64 * (HASHMAP_BUCKET_COST + ENTRY_BASE_COST);
        for entry in self.sessions.values() {
            // TimelineEntry structs (stack/arena size).
            struct_bytes += (entry.timeline.len() * std::mem::size_of::<TimelineEntry>()) as u64;
            // Vec<u64> ×2 actual element storage.
            struct_bytes += (entry.request_ids.len() * std::mem::size_of::<u64>()) as u64;
            struct_bytes += (entry.last_fingerprints.len() * std::mem::size_of::<u64>()) as u64;
            // Vec heap overhead (capacity * pointer-width, the allocated region).
            struct_bytes += (entry.timeline.capacity() * VEC_HEAP_PER_SLOT) as u64;
            struct_bytes += (entry.request_ids.capacity() * VEC_HEAP_PER_SLOT) as u64;
            struct_bytes += (entry.last_fingerprints.capacity() * VEC_HEAP_PER_SLOT) as u64;
            // String structs on stack; actual buffer counted in metadata_bytes.
            struct_bytes += std::mem::size_of::<String>() as u64 * 4; // id, scope.0, scope.1, match_reason
            if entry.title.is_some() {
                struct_bytes += std::mem::size_of::<String>() as u64;
            }
            if entry.source.is_some() {
                struct_bytes += std::mem::size_of::<String>() as u64;
            }
        }

        let total_est_bytes =
            timeline_content_bytes + metadata_bytes + fingerprint_bytes + struct_bytes;

        BackendMemoryStats {
            session_count,
            max_sessions: self.max_sessions,
            timeline_entry_count,
            timeline_content_bytes,
            metadata_bytes,
            fingerprint_bytes,
            struct_bytes,
            total_est_bytes,
        }
    }
}

fn session_key(scope: &(String, String), header_val: &str) -> String {
    format!("{}|{}|{}", scope.0, scope.1, header_val)
}

/// 单 turn 指纹：role + content 序列化文本的哈希。
/// 同会话续轮时客户端原样重发历史，同一输入的序列化输出稳定，
/// 指纹相等即可代表 turn 相等（碰撞概率可忽略；仅内存态，不持久化）。
pub(crate) fn turn_fingerprint(turn: &AiTurn) -> u64 {
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
