use rquickjs::{Context, Runtime, Value};

use super::data::{RequestData, ResponseData};

pub(super) fn exec_request_hook(
    script: &str,
    data: &RequestData,
) -> Result<Option<RequestData>, String> {
    let rt = Runtime::new().map_err(|e| e.to_string())?;
    let ctx = Context::full(&rt).map_err(|e| e.to_string())?;

    ctx.with(|ctx| {
        let data_json = serde_json::to_string(data).expect("JSON serialization of RequestData should always succeed");
        let req_obj: Value = ctx
            .eval::<Value, _>(format!("({data_json})"))
            .map_err(|e| format!("parse request JSON: {e}"))?;

        let globals = ctx.globals();
        globals.set("__req", req_obj).map_err(|e| e.to_string())?;

        // inject log via eval to avoid moving ctx (Function::new takes ctx by value)
        ctx.eval::<(), _>("var __logs = []; function log(m) { __logs.push(String(m)); }")
            .map_err(|e| format!("setup log: {e}"))?;

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
            let result: Value = ctx
                .eval("onRequest(__req)")
                .map_err(|e| format!("onRequest: {e}"))?;
            if result.is_null() || result.is_undefined() {
                return Ok(None);
            }
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
        let data_json = serde_json::to_string(data).expect("JSON serialization of ResponseData should always succeed");
        let res_obj: Value = ctx
            .eval::<Value, _>(format!("({data_json})"))
            .map_err(|e| format!("parse response JSON: {e}"))?;

        let globals = ctx.globals();
        globals.set("__res", res_obj).map_err(|e| e.to_string())?;

        ctx.eval::<(), _>("var __logs = []; function log(m) { __logs.push(String(m)); }")
            .map_err(|e| format!("setup log: {e}"))?;

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
            ctx.eval::<(), _>("onResponse(__res)")
                .map_err(|e| format!("onResponse: {e}"))?;
        }

        let result_json: String = ctx
            .eval("JSON.stringify(__res)")
            .map_err(|e| format!("stringify result: {e}"))?;
        let modified: ResponseData =
            serde_json::from_str(&result_json).map_err(|e| format!("parse result: {e}"))?;
        Ok(modified)
    })
}
