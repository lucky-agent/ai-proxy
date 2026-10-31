import { invoke } from "@tauri-apps/api/core";
import { ArrowRightIcon, PencilIcon, Trash2Icon } from "lucide-react";
import { useEffect, useState } from "react";
import { ConfigDialog } from "@/components/core/ConfigDialog";
import { ListSection } from "@/components/core/ListSection";
import { Checkbox } from "@/components/ui/checkbox";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useLocale } from "@/hooks/useLocale";
import { useSlidePanel } from "@/hooks/useSlidePanel";
import { cn } from "@/lib/utils";
import type { ReverseConfig, ReverseProxyRule } from "@/types/settings";
import ReverseRuleEditForm from "./ReverseRuleEditForm";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** 四列网格：表头与数据行共用，保证列对齐 */
const ROW_GRID = "flex items-center gap-2.5 px-3";
const COL = {
  enabled: "flex w-4 shrink-0 justify-start",
  local: "min-w-0 flex-1",
  arrow: "flex w-5 shrink-0 items-center justify-center",
  upstream: "min-w-0 flex-1",
  actions: "flex w-[46px] shrink-0 items-center justify-end gap-0.5",
};

const HEADER_CELL =
  "overflow-visible text-ui-xs font-medium tracking-wider whitespace-nowrap text-muted-foreground uppercase";

/** 本地监听展示：`host:port`（端口未填时只显示 host） */
function localLabel(rule: ReverseProxyRule): string {
  const host = rule.listen_host.trim() || "127.0.0.1";
  return rule.listen_port > 0 ? `${host}:${rule.listen_port}` : host;
}

/** 上游展示：`scheme://host[:port]`，默认端口省略，IPv6 加方括号 */
function upstreamLabel(rule: ReverseProxyRule): string {
  const scheme = rule.upstream_scheme.trim() || "https";
  const host = rule.upstream_host.trim();
  if (!host) return "—";
  const bracketed = host.includes(":") && !host.startsWith("[") ? `[${host}]` : host;
  const defaultPort = scheme === "https" ? 443 : 80;
  const port = rule.upstream_port === 0 || rule.upstream_port === defaultPort ? "" : `:${rule.upstream_port}`;
  return `${scheme}://${bracketed}${port}`;
}

export default function ReverseConfigDialog({ open, onOpenChange }: Props) {
  const { t } = useLocale();
  const [enabled, setEnabled] = useState(false);
  const [rules, setRules] = useState<ReverseProxyRule[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  // 侧滑面板状态：index null = 新增，数字 = 编辑对应行
  const panel = useSlidePanel();

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    setError("");
    panel.reset();
    invoke<ReverseConfig>("get_reverse_config")
      .then((config) => {
        setEnabled(config.enabled);
        setRules(config.rules);
      })
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, [open, panel.reset]);

  function toggleItem(index: number) {
    setRules((prev) => {
      const updated = [...prev];
      updated[index] = { ...updated[index], enabled: !updated[index].enabled };
      return updated;
    });
  }

  function removeItem(index: number) {
    setRules((prev) => prev.filter((_, i) => i !== index));
  }

  /** 编辑器「确定」：新增追加（默认启用），编辑原位替换（保留启用状态） */
  function applyRule(draft: Omit<ReverseProxyRule, "enabled">) {
    const target = panel.target;
    setRules((prev) => {
      if (target === null) return prev;
      if (target.index === null) return [...prev, { ...draft, enabled: true }];
      const updated = [...prev];
      updated[target.index] = { ...draft, enabled: updated[target.index].enabled };
      return updated;
    });
    panel.close();
  }

  async function handleSave() {
    setSaving(true);
    setError("");
    const cleaned = rules.map((r) => ({
      ...r,
      listen_host: r.listen_host.trim(),
      upstream_scheme: r.upstream_scheme.trim(),
      upstream_host: r.upstream_host.trim(),
    }));
    try {
      await invoke("save_reverse_config", { reverse: { enabled, rules: cleaned } });
      setRules(cleaned);
      onOpenChange(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  const editIndex = panel.target?.index ?? null;
  const editingRule = editIndex !== null ? (rules[editIndex] ?? null) : null;

  return (
    <ConfigDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("reverseConfig.title")}
      description={t("reverseConfig.description")}
      enabled={enabled}
      onEnabledChange={setEnabled}
      widthClass="sm:max-w-[620px]"
      error={error}
      loading={loading}
      saving={saving}
      onSave={handleSave}
      panel={panel.target && <ReverseRuleEditForm initial={editingRule} onConfirm={applyRule} onCancel={panel.close} />}
      panelClosing={panel.closing}
      onPanelDismiss={panel.close}
    >
      <ListSection
        title={t("reverseConfig.ruleListHeader")}
        countLabel={t("reverseConfig.ruleCount", { n: rules.length })}
        addLabel={t("reverseConfig.addRule")}
        onAdd={() => panel.open(null)}
        loading={loading}
        loadingLabel={t("settings.loading")}
        empty={rules.length === 0}
        emptyLabel={t("reverseConfig.emptyList")}
        // 面板打开时外壳已整体变暗，不再叠加禁用态变暗
        disabled={!enabled && !panel.target}
        className="select-none"
      >
        <div
          className={cn(
            ROW_GRID,
            "sticky top-0 z-10 h-7 border-b border-border bg-[color-mix(in_oklab,var(--popover),var(--foreground)_3%)]",
          )}
        >
          <span className={cn(COL.enabled, HEADER_CELL)}>{t("reverseConfig.colEnabled")}</span>
          <span className={cn(COL.local, HEADER_CELL, "pl-1.5")}>{t("reverseConfig.colLocal")}</span>
          <span className={COL.arrow} />
          <span className={cn(COL.upstream, HEADER_CELL)}>{t("reverseConfig.colUpstream")}</span>
          <span className={COL.actions} />
        </div>

        {rules.map((rule, index) => (
          <div
            // biome-ignore lint/suspicious/noArrayIndexKey: rules have no stable id (index-based editing)
            key={index}
            className={cn(
              ROW_GRID,
              "group/row h-[34px] cursor-pointer border-b border-border transition-colors last:border-b-0 hover:bg-foreground/[0.035]",
            )}
            onDoubleClick={() => panel.open(index)}
          >
            <span className={COL.enabled} onClick={(e) => e.stopPropagation()}>
              <Checkbox checked={rule.enabled} onCheckedChange={() => toggleItem(index)} />
            </span>
            <span className={cn(COL.local, "truncate font-mono text-xs", !rule.enabled && "opacity-40")}>
              {localLabel(rule)}
            </span>
            <span className={cn(COL.arrow, !rule.enabled && "opacity-40")}>
              <ArrowRightIcon className="size-3.5 text-muted-foreground/60" />
            </span>
            <span className={cn(COL.upstream, "truncate font-mono text-xs", !rule.enabled && "opacity-40")}>
              {upstreamLabel(rule)}
            </span>
            <span className={COL.actions}>
              <Tooltip>
                <TooltipTrigger className="inline-flex">
                  <button
                    type="button"
                    className="inline-flex size-[22px] items-center justify-center rounded-md text-muted-foreground opacity-0 transition-colors group-hover/row:opacity-100 hover:bg-muted hover:text-foreground focus-visible:opacity-100"
                    onClick={(e) => {
                      e.stopPropagation();
                      panel.open(index);
                    }}
                  >
                    <PencilIcon className="size-3" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="top" className="bg-popover text-popover-foreground text-ui-sm">
                  {t("reverseConfig.editRule")}
                </TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger className="inline-flex">
                  <button
                    type="button"
                    className="inline-flex size-[22px] items-center justify-center rounded-md text-muted-foreground opacity-0 transition-colors group-hover/row:opacity-100 hover:bg-destructive/10 hover:text-destructive focus-visible:opacity-100"
                    onClick={(e) => {
                      e.stopPropagation();
                      removeItem(index);
                    }}
                  >
                    <Trash2Icon className="size-3.5" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="top" className="bg-popover text-popover-foreground text-ui-sm">
                  {t("reverseConfig.delete")}
                </TooltipContent>
              </Tooltip>
            </span>
          </div>
        ))}
      </ListSection>
    </ConfigDialog>
  );
}
