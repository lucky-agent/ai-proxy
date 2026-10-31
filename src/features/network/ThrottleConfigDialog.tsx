import { invoke } from "@tauri-apps/api/core";
import { ArrowDownIcon, ArrowUpIcon, Trash2Icon } from "lucide-react";
import { useEffect, useState } from "react";
import { ConfigDialog } from "@/components/core/ConfigDialog";
import { ListSection } from "@/components/core/ListSection";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useLocale } from "@/hooks/useLocale";
import { notifyConfigChanged } from "@/lib/config-events";
import { cn } from "@/lib/utils";
import type { ThrottleConfig, ThrottleParams, ThrottleProfile, ThrottleRule } from "@/types/settings";
import { formatThrottleParams, newThrottleRule, ruleParamsOf, THROTTLE_PRESET_ITEMS } from "./throttle-presets";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /**
   * 从菜单「启用网络限速」进入且尚无可用规则：总开关预置为开，列表为空时预填一条 `*` 规则。
   * 仅影响弹窗内的草稿，点保存才生效。
   */
  enableOnOpen?: boolean;
}

const ROW_GRID = "flex items-center gap-2.5 px-3";
const COL = {
  enabled: "flex w-4 shrink-0 justify-start",
  domain: "min-w-0 flex-1",
  profile: "w-32 shrink-0",
  actions: "flex w-[70px] shrink-0 items-center justify-end gap-0.5",
};

const HEADER_CELL =
  "overflow-visible text-ui-xs font-medium tracking-wider whitespace-nowrap text-muted-foreground uppercase";

const FIELD_INPUT = "h-7 rounded-md border-input px-2 text-xs transition-colors focus-visible:ring-1";

const ACTION_BTN =
  "inline-flex size-[22px] items-center justify-center rounded-md text-muted-foreground opacity-0 transition-colors group-hover/row:opacity-100 hover:bg-muted hover:text-foreground focus-visible:opacity-100 disabled:pointer-events-none disabled:opacity-0";

const CUSTOM_FIELDS = [
  { key: "download_kbps", labelKey: "network.download", unit: "kbps" },
  { key: "upload_kbps", labelKey: "network.upload", unit: "kbps" },
  { key: "latency_ms", labelKey: "network.latency", unit: "ms" },
] as const;

function IconAction({
  label,
  onClick,
  disabled,
  destructive,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  destructive?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger className="inline-flex" render={<span />}>
        <button
          type="button"
          aria-label={label}
          disabled={disabled}
          className={cn(ACTION_BTN, destructive && "hover:bg-destructive/10 hover:text-destructive")}
          onClick={onClick}
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent side="top" className="bg-popover text-popover-foreground text-ui-sm">
        {label}
      </TooltipContent>
    </Tooltip>
  );
}

/** 网络节流规则弹窗：总开关 + 域名规则列表（自上而下命中第一条），行内直接编辑。 */
export default function ThrottleConfigDialog({ open, onOpenChange, enableOnOpen = false }: Props) {
  const { t } = useLocale();
  const [enabled, setEnabled] = useState(false);
  const [rules, setRules] = useState<ThrottleRule[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    setError("");
    invoke<ThrottleConfig>("get_throttle_config")
      .then((cfg) => {
        setEnabled(cfg.enabled || enableOnOpen);
        setRules(enableOnOpen && cfg.rules.length === 0 ? [{ ...newThrottleRule(), domain: "*" }] : cfg.rules);
      })
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, [open, enableOnOpen]);

  function updateRule(index: number, patch: Partial<ThrottleRule>) {
    setRules((prev) => prev.map((r, i) => (i === index ? { ...r, ...patch } : r)));
    setError("");
  }

  function updateCustom(index: number, key: keyof ThrottleParams, value: string) {
    const n = Math.max(0, Math.floor(Number(value) || 0));
    setRules((prev) => prev.map((r, i) => (i === index ? { ...r, custom: { ...r.custom, [key]: n } } : r)));
  }

  function move(index: number, delta: -1 | 1) {
    setRules((prev) => {
      const target = index + delta;
      if (target < 0 || target >= prev.length) return prev;
      const next = [...prev];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  }

  function removeRule(index: number) {
    setRules((prev) => prev.filter((_, i) => i !== index));
  }

  async function handleSave() {
    const cleaned = rules.map((r) => ({ ...r, domain: r.domain.trim() }));
    if (cleaned.some((r) => r.enabled && !r.domain)) {
      setError(t("network.domainRequired"));
      return;
    }
    setSaving(true);
    setError("");
    try {
      await invoke("save_throttle_config", { throttle: { enabled, rules: cleaned } satisfies ThrottleConfig });
      notifyConfigChanged();
      onOpenChange(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  const profileLabel = (p: ThrottleProfile) =>
    p === "none" ? t("network.unlimited") : p === "custom" ? t("network.customShort") : p;

  return (
    <ConfigDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("network.rulesTitle")}
      description={t("network.hint")}
      enabled={enabled}
      onEnabledChange={setEnabled}
      widthClass="sm:max-w-[620px]"
      error={error}
      loading={loading}
      saving={saving}
      onSave={handleSave}
    >
      <ListSection
        title={t("network.ruleListHeader")}
        countLabel={t("network.ruleCount", { n: rules.length })}
        addLabel={t("network.addRule")}
        onAdd={() => setRules((prev) => [...prev, newThrottleRule()])}
        loading={loading}
        loadingLabel={t("settings.loading")}
        empty={rules.length === 0}
        emptyLabel={t("network.emptyList")}
        disabled={!enabled}
      >
        <div
          className={cn(
            ROW_GRID,
            "sticky top-0 z-10 h-7 border-b border-border bg-[color-mix(in_oklab,var(--popover),var(--foreground)_3%)]",
          )}
        >
          <span className={cn(COL.enabled, HEADER_CELL)}>{t("network.colEnabled")}</span>
          <span className={cn(COL.domain, HEADER_CELL, "pl-1")}>{t("network.colDomain")}</span>
          <span className={cn(COL.profile, HEADER_CELL, "pl-1")}>{t("network.colProfile")}</span>
          <span className={COL.actions} />
        </div>

        {rules.map((rule, index) => {
          const params = ruleParamsOf(rule);
          return (
            <div
              // biome-ignore lint/suspicious/noArrayIndexKey: rules have no stable id (index-based editing)
              key={index}
              className="group/row border-b border-border py-1.5 transition-colors last:border-b-0 hover:bg-foreground/[0.035]"
            >
              <div className={ROW_GRID}>
                <span className={COL.enabled}>
                  <Checkbox
                    checked={rule.enabled}
                    onCheckedChange={() => updateRule(index, { enabled: !rule.enabled })}
                  />
                </span>
                <span className={cn(COL.domain, !rule.enabled && "opacity-40")}>
                  <Input
                    className={cn(FIELD_INPUT, "font-mono")}
                    value={rule.domain}
                    placeholder={t("network.placeholderDomain")}
                    onChange={(e) => updateRule(index, { domain: e.target.value })}
                  />
                </span>
                <span className={cn(COL.profile, !rule.enabled && "opacity-40")}>
                  <Tooltip>
                    <TooltipTrigger className="block w-full" render={<span />}>
                      <Select
                        value={rule.profile}
                        onValueChange={(v) => v && updateRule(index, { profile: v as ThrottleProfile })}
                      >
                        <SelectTrigger className={cn(FIELD_INPUT, "w-full")}>
                          <SelectValue>{(v: ThrottleProfile) => profileLabel(v)}</SelectValue>
                        </SelectTrigger>
                        <SelectContent className="min-w-32">
                          {THROTTLE_PRESET_ITEMS.map(({ value, label }) => (
                            <SelectItem key={value} value={value}>
                              {label}
                            </SelectItem>
                          ))}
                          <SelectItem value="custom">{t("network.customShort")}</SelectItem>
                          <SelectItem value="none">{t("network.unlimited")}</SelectItem>
                        </SelectContent>
                      </Select>
                    </TooltipTrigger>
                    {params && (
                      <TooltipContent side="top" className="bg-popover font-mono text-popover-foreground text-ui-xs">
                        {formatThrottleParams(params)}
                      </TooltipContent>
                    )}
                  </Tooltip>
                </span>
                <span className={COL.actions}>
                  <IconAction label={t("network.moveUp")} disabled={index === 0} onClick={() => move(index, -1)}>
                    <ArrowUpIcon className="size-3.5" />
                  </IconAction>
                  <IconAction
                    label={t("network.moveDown")}
                    disabled={index === rules.length - 1}
                    onClick={() => move(index, 1)}
                  >
                    <ArrowDownIcon className="size-3.5" />
                  </IconAction>
                  <IconAction label={t("network.delete")} destructive onClick={() => removeRule(index)}>
                    <Trash2Icon className="size-3.5" />
                  </IconAction>
                </span>
              </div>

              {rule.profile === "custom" && (
                <div className={cn(ROW_GRID, "mt-1.5 pl-[38px]", !rule.enabled && "opacity-40")}>
                  {CUSTOM_FIELDS.map(({ key, labelKey, unit }) => (
                    <label key={key} className="flex min-w-0 flex-1 items-center gap-1.5">
                      <span className="shrink-0 text-ui-xs text-muted-foreground">
                        {t(labelKey)}
                        <span className="ml-0.5 text-muted-foreground/70">({unit})</span>
                      </span>
                      <Input
                        className={cn(FIELD_INPUT, "no-spinner min-w-0")}
                        type="number"
                        min={0}
                        value={rule.custom[key]}
                        onChange={(e) => updateCustom(index, key, e.target.value)}
                      />
                    </label>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </ListSection>
      <p className="mt-2 text-ui-sm text-muted-foreground">{t("network.orderNote")}</p>
    </ConfigDialog>
  );
}
