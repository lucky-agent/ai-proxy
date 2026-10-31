import type { ThrottleParams, ThrottleProfile, ThrottleRule } from "@/types/settings";

type PresetProfile = Exclude<ThrottleProfile, "none" | "custom">;

/**
 * 网络节流预设档位参数，与后端 `ThrottleProfile::preset`（src-tauri/src/config/settings.rs）保持一致，
 * 前端仅用于展示，实际生效值以后端为准。
 * 3G / Slow 4G / Fast 4G 与 Chrome DevTools 预设一致；2G 取 Firefox Regular 2G 按同样系数换算。
 */
export const THROTTLE_PRESETS: Record<PresetProfile, ThrottleParams> = {
  "2g": { download_kbps: 200, upload_kbps: 40, latency_ms: 1500 },
  "3g": { download_kbps: 400, upload_kbps: 400, latency_ms: 2000 },
  slow4g: { download_kbps: 1440, upload_kbps: 675, latency_ms: 562 },
  fast4g: { download_kbps: 8100, upload_kbps: 1350, latency_ms: 165 },
  // Chrome / Firefox 均无 5G 预设，取估算的典型值
  "5g": { download_kbps: 100_000, upload_kbps: 30_000, latency_ms: 20 },
};

export const DEFAULT_CUSTOM_THROTTLE: ThrottleParams = { download_kbps: 1000, upload_kbps: 500, latency_ms: 100 };

/** 预设档位的顺序与名称（2G/3G/4G 名称不翻译） */
export const THROTTLE_PRESET_ITEMS: { value: PresetProfile; label: string }[] = [
  { value: "2g", label: "2G" },
  { value: "3g", label: "3G" },
  { value: "slow4g", label: "Slow 4G" },
  { value: "fast4g", label: "Fast 4G" },
  { value: "5g", label: "5G" },
];

/** 预设档位显示名；不限速 / 自定义返回 null（由调用方翻译） */
export function presetLabelOf(profile: ThrottleProfile): string | null {
  return THROTTLE_PRESET_ITEMS.find((p) => p.value === profile)?.label ?? null;
}

/** 规则实际生效的参数；不限速时为 null */
export function ruleParamsOf(rule: Pick<ThrottleRule, "profile" | "custom">): ThrottleParams | null {
  if (rule.profile === "none") return null;
  return rule.profile === "custom" ? rule.custom : THROTTLE_PRESETS[rule.profile];
}

/** kbps → 可读文本：≥1000 显示 Mbps，0 显示 ∞（不限速） */
export function formatKbps(kbps: number): string {
  if (kbps <= 0) return "∞";
  return kbps >= 1000 ? `${Number((kbps / 1000).toFixed(2))} Mbps` : `${kbps} kbps`;
}

/** 参数摘要，如 "↓400 kbps  ↑400 kbps  2000ms" */
export function formatThrottleParams(p: ThrottleParams): string {
  return `↓${formatKbps(p.download_kbps)}  ↑${formatKbps(p.upload_kbps)}  ${p.latency_ms}ms`;
}

/** 新增规则的默认值 */
export function newThrottleRule(): ThrottleRule {
  return { enabled: true, domain: "", profile: "3g", custom: { ...DEFAULT_CUSTOM_THROTTLE } };
}
