/**
 * 配置变更通知：任何写配置的地方（弹窗保存、脚本编辑器、解密提示等）成功后调用
 * `notifyConfigChanged()`，App 监听后刷新底部栏的 TLS / 脚本 / AI 开关状态，保证各入口联动。
 */
const EVENT = "ai-proxy:config-changed";

export function notifyConfigChanged() {
  window.dispatchEvent(new Event(EVENT));
}

export function onConfigChanged(cb: () => void): () => void {
  window.addEventListener(EVENT, cb);
  return () => window.removeEventListener(EVENT, cb);
}
