use rquickjs::{Context, Runtime, Value};

use super::data::{RequestData, ResponseData};

/// 脚本执行前注入的公共代码：
/// - `log`：收集脚本日志，执行完后统一输出
/// - `__decodeBody`：按 content-type 将 body 解析为对象，脚本可直接 `body["key"] = ...`：
///   - 含 `json` 且可解析为对象/数组 → JSON 对象
///   - `application/x-www-form-urlencoded` → `{ key: value }`，同名 key 合并为数组
///   - 其余情况保持字符串
/// - `__encodeBody`：hook 执行后 body 若不是字符串，则按解析时的格式序列化回去
///   （脚本自行赋值的对象按 JSON）；内容未被修改时还原原始字符串，
///   避免格式被压缩、大整数精度丢失、编码风格变化
const PRELUDE: &str = r#"
var __logs = [];
function log(m) { __logs.push(String(m)); }
var __bodyKind = null, __rawBody = null, __rawSnapshot = null;
function __formDecode(s) {
    var out = {};
    s.split("&").forEach(function (pair) {
        if (pair === "") return;
        var i = pair.indexOf("=");
        var k = i === -1 ? pair : pair.slice(0, i);
        var v = i === -1 ? "" : pair.slice(i + 1);
        k = decodeURIComponent(k.replace(/\+/g, " "));
        v = decodeURIComponent(v.replace(/\+/g, " "));
        if (!Object.prototype.hasOwnProperty.call(out, k)) out[k] = v;
        else if (Array.isArray(out[k])) out[k].push(v);
        else out[k] = [out[k], v];
    });
    return out;
}
function __formEncode(o) {
    var enc = function (s) { return encodeURIComponent(String(s)).replace(/%20/g, "+"); };
    var parts = [];
    Object.keys(o).forEach(function (k) {
        var v = o[k];
        if (v === undefined || v === null) return;
        (Array.isArray(v) ? v : [v]).forEach(function (x) { parts.push(enc(k) + "=" + enc(x)); });
    });
    return parts.join("&");
}
function __decodeBody(o) {
    var ct = (o.headers["content-type"] || "").toLowerCase();
    try {
        if (ct.indexOf("json") !== -1) {
            var v = JSON.parse(o.body);
            if (v === null || typeof v !== "object") return;
            __bodyKind = "json";
            __rawSnapshot = JSON.stringify(v);
        } else if (ct.indexOf("application/x-www-form-urlencoded") !== -1) {
            var v = __formDecode(o.body);
            __bodyKind = "form";
            __rawSnapshot = __formEncode(v);
        } else {
            return;
        }
    } catch (_) { return; }
    __rawBody = o.body;
    o.body = v;
}
function __encodeBody(o) {
    if (typeof o.body === "string") return;
    if (o.body === undefined || o.body === null) { o.body = ""; return; }
    var s = __bodyKind === "form" ? __formEncode(o.body) : JSON.stringify(o.body);
    o.body = s === __rawSnapshot ? __rawBody : s;
}
"#;

pub(super) fn exec_request_hook(
    script: &str,
    data: &RequestData,
) -> Result<Option<RequestData>, String> {
    let rt = Runtime::new().map_err(|e| e.to_string())?;
    let ctx = Context::full(&rt).map_err(|e| e.to_string())?;

    ctx.with(|ctx| {
        let data_json = serde_json::to_string(data)
            .expect("JSON serialization of RequestData should always succeed");
        let req_obj: Value = ctx
            .eval::<Value, _>(format!("({data_json})"))
            .map_err(|e| format!("parse request JSON: {e}"))?;

        let globals = ctx.globals();
        globals.set("__req", req_obj).map_err(|e| e.to_string())?;

        // inject helpers via eval to avoid moving ctx (Function::new takes ctx by value)
        ctx.eval::<(), _>(PRELUDE)
            .map_err(|e| format!("setup prelude: {e}"))?;

        ctx.eval::<(), _>(script)
            .map_err(|e| format!("script error: {e}"))?;

        // flush collected logs
        let log_count: i32 = ctx.eval("__logs.length").unwrap_or(0);
        for i in 0..log_count {
            let msg: String = ctx.eval(format!("__logs[{i}]")).unwrap_or_default();
            log::info!("[script] {msg}");
        }

        let has_hook: bool = ctx.eval("typeof onRequest === 'function'").unwrap_or(false);
        if has_hook {
            ctx.eval::<(), _>("__decodeBody(__req)")
                .map_err(|e| format!("decode body: {e}"))?;
            let result: Value = ctx
                .eval("onRequest(__req)")
                .map_err(|e| format!("onRequest: {e}"))?;
            // 支持 async function：等待返回的 Promise 完成，取回实际返回值
            let resolved: Value = match result.as_promise() {
                Some(promise) => promise
                    .finish()
                    .map_err(|e| format!("onRequest await: {e}"))?,
                None => result,
            };
            // 返回 null/undefined 表示阻止该请求
            if resolved.is_null() || resolved.is_undefined() {
                return Ok(None);
            }
            ctx.eval::<(), _>("__encodeBody(__req)")
                .map_err(|e| format!("encode body: {e}"))?;
        }

        let result_json: String = ctx
            .eval("JSON.stringify(__req)")
            .map_err(|e| format!("stringify result: {e}"))?;
        let modified: RequestData =
            serde_json::from_str(&result_json).map_err(|e| format!("parse result: {e}"))?;
        Ok(Some(modified))
    })
}

pub(super) fn exec_response_hook(
    script: &str,
    data: &ResponseData,
) -> Result<ResponseData, String> {
    let rt = Runtime::new().map_err(|e| e.to_string())?;
    let ctx = Context::full(&rt).map_err(|e| e.to_string())?;

    ctx.with(|ctx| {
        let data_json = serde_json::to_string(data)
            .expect("JSON serialization of ResponseData should always succeed");
        let res_obj: Value = ctx
            .eval::<Value, _>(format!("({data_json})"))
            .map_err(|e| format!("parse response JSON: {e}"))?;

        let globals = ctx.globals();
        globals.set("__res", res_obj).map_err(|e| e.to_string())?;

        ctx.eval::<(), _>(PRELUDE)
            .map_err(|e| format!("setup prelude: {e}"))?;

        ctx.eval::<(), _>(script)
            .map_err(|e| format!("script error: {e}"))?;

        let log_count: i32 = ctx.eval("__logs.length").unwrap_or(0);
        for i in 0..log_count {
            let msg: String = ctx.eval(format!("__logs[{i}]")).unwrap_or_default();
            log::info!("[script] {msg}");
        }

        let has_hook: bool = ctx
            .eval("typeof onResponse === 'function'")
            .unwrap_or(false);
        if has_hook {
            ctx.eval::<(), _>("__decodeBody(__res)")
                .map_err(|e| format!("decode body: {e}"))?;
            let result: Value = ctx
                .eval("onResponse(__res)")
                .map_err(|e| format!("onResponse: {e}"))?;
            // 支持 async function：等待 Promise 完成，确保 await 之后的修改在序列化前生效
            if let Some(promise) = result.as_promise() {
                promise
                    .finish::<Value>()
                    .map_err(|e| format!("onResponse await: {e}"))?;
            }
            ctx.eval::<(), _>("__encodeBody(__res)")
                .map_err(|e| format!("encode body: {e}"))?;
        }

        let result_json: String = ctx
            .eval("JSON.stringify(__res)")
            .map_err(|e| format!("stringify result: {e}"))?;
        let modified: ResponseData =
            serde_json::from_str(&result_json).map_err(|e| format!("parse result: {e}"))?;
        Ok(modified)
    })
}
