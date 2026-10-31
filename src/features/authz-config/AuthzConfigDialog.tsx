import { invoke } from "@tauri-apps/api/core";
import { PencilIcon, Trash2Icon } from "lucide-react";
import { useEffect, useState } from "react";
import { ConfigDialog } from "@/components/core/ConfigDialog";
import { ListSection } from "@/components/core/ListSection";
import { Checkbox } from "@/components/ui/checkbox";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useLocale } from "@/hooks/useLocale";
import { useSlidePanel } from "@/hooks/useSlidePanel";
import { METHOD_BG_COLORS } from "@/lib/http-constants";
import { cn } from "@/lib/utils";
import type { AuthzConfig, AuthzRule } from "@/types/settings";
import AuthzRuleEditForm from "./AuthzRuleEditForm";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** 五列网格：表头与数据行共用，保证列对齐（与 AI 检测 / 反向代理配置同构） */
const ROW_GRID = "flex items-center gap-2.5 px-3";
const COL = {
  enabled: "flex w-4 shrink-0 justify-start",
  name: "min-w-0 w-[24%] shrink-0",
  match: "min-w-0 flex-1",
  url: "min-w-0 flex-1",
  onError: "w-12 shrink-0",
  actions: "flex w-[46px] shrink-0 items-center justify-end gap-0.5",
};

const HEADER_CELL =
  "overflow-visible text-ui-xs font-medium tracking-wider whitespace-nowrap text-muted-foreground uppercase";

const ACTION_BTN =
  "inline-flex size-[22px] items-center justify-center rounded-md text-muted-foreground opacity-0 transition-colors group-hover/row:opacity-100 focus-visible:opacity-100";

/** 带 tooltip 的单行截断文本（列宽有限时悬停看全文） */
function TruncCell({ text, mono }: { text: string; mono?: boolean }) {
  return (
    <Tooltip>
      <TooltipTrigger className="block w-full min-w-0 text-left">
        <span className={cn("block truncate text-xs", mono && "font-mono")}>{text}</span>
      </TooltipTrigger>
      <TooltipContent
        side="top"
        align="start"
        className={cn("max-w-[420px] bg-popover text-popover-foreground text-ui-sm", mono && "font-mono")}
      >
        {text}
      </TooltipContent>
    </Tooltip>
  );
}

export default function AuthzConfigDialog({ open, onOpenChange }: Props) {
  const { t } = useLocale();
  const [enabled, setEnabled] = useState(false);
  const [rules, setRules] = useState<AuthzRule[]>([]);
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
    invoke<AuthzConfig>("get_authz_config")
      .then((config) => {
        setEnabled(config.enabled);
        setRules(config.rules);
      })
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, [open, panel.reset]);

  function toggleRule(index: number) {
    setRules((prev) => {
      const updated = [...prev];
      updated[index] = { ...updated[index], enabled: !updated[index].enabled };
      return updated;
    });
  }

  function removeRule(index: number) {
    setRules((prev) => prev.filter((_, i) => i !== index));
  }

  /** 编辑器「确定」：新增追加（默认启用），编辑原位替换（保留启用状态） */
  function applyRule(draft: Omit<AuthzRule, "enabled">) {
    const target = panel.target;
    setError("");
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
    setError("");
    // 表单确定时已校验；这里兜底旧配置里可能存在的不完整规则
    for (const r of rules) {
      if (r.enabled && r.name.trim() === "") {
        setError(t("authzConfig.nameRequired"));
        return;
      }
      if (r.enabled && r.url.trim() === "") {
        setError(t("authzConfig.urlRequired"));
        return;
      }
    }
    setSaving(true);
    try {
      await invoke("save_authz_config", { authz: { enabled, rules } });
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
      title={t("authzConfig.title")}
      description={t("authzConfig.description")}
      enabled={enabled}
      onEnabledChange={setEnabled}
      widthClass="sm:max-w-[640px]"
      error={error}
      loading={loading}
      saving={saving}
      onSave={handleSave}
      panel={panel.target && <AuthzRuleEditForm initial={editingRule} onConfirm={applyRule} onCancel={panel.close} />}
      panelClosing={panel.closing}
      onPanelDismiss={panel.close}
    >
      <ListSection
        title={t("authzConfig.ruleListHeader")}
        countLabel={t("authzConfig.ruleCount", { n: rules.length })}
        addLabel={t("authzConfig.addRule")}
        onAdd={() => panel.open(null)}
        loading={loading}
        loadingLabel={t("settings.loading")}
        empty={rules.length === 0}
        emptyLabel={t("authzConfig.emptyList")}
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
          <span className={cn(COL.enabled, HEADER_CELL)}>{t("authzConfig.colEnabled")}</span>
          <span className={cn(COL.name, HEADER_CELL)}>{t("authzConfig.colName")}</span>
          <span className={cn(COL.match, HEADER_CELL)}>{t("authzConfig.colMatch")}</span>
          <span className={cn(COL.url, HEADER_CELL)}>{t("authzConfig.colUrl")}</span>
          <span className={cn(COL.onError, HEADER_CELL)}>{t("authzConfig.colOnError")}</span>
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
              <Checkbox checked={rule.enabled} onCheckedChange={() => toggleRule(index)} />
            </span>
            <span className={cn(COL.name, !rule.enabled && "opacity-40")}>
              <TruncCell text={rule.name || "—"} />
            </span>
            <span className={cn(COL.match, "flex items-center gap-1.5", !rule.enabled && "opacity-40")}>
              <span
                className="shrink-0 text-ui-sm font-medium"
                style={{
                  color: rule.method
                    ? (METHOD_BG_COLORS as Record<string, string>)[rule.method]
                    : "var(--muted-foreground)",
                }}
              >
                {rule.method || "Any"}
              </span>
              <span className="min-w-0 flex-1">
                <TruncCell text={rule.domain || "*"} mono />
              </span>
            </span>
            <span className={cn(COL.url, !rule.enabled && "opacity-40")}>
              <TruncCell text={rule.url || "—"} mono />
            </span>
            <span
              className={cn(
                COL.onError,
                "text-ui-sm",
                rule.on_error === "open" ? "text-muted-foreground" : "text-foreground",
                !rule.enabled && "opacity-40",
              )}
            >
              {rule.on_error === "open" ? t("authzConfig.onErrorOpenShort") : t("authzConfig.onErrorClosedShort")}
            </span>
            <span className={COL.actions}>
              <Tooltip>
                <TooltipTrigger className="inline-flex">
                  <button
                    type="button"
                    className={cn(ACTION_BTN, "hover:bg-muted hover:text-foreground")}
                    onClick={(e) => {
                      e.stopPropagation();
                      panel.open(index);
                    }}
                  >
                    <PencilIcon className="size-3" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="top" className="bg-popover text-popover-foreground text-ui-sm">
                  {t("authzConfig.editRule")}
                </TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger className="inline-flex">
                  <button
                    type="button"
                    className={cn(ACTION_BTN, "hover:bg-destructive/10 hover:text-destructive")}
                    onClick={(e) => {
                      e.stopPropagation();
                      removeRule(index);
                    }}
                  >
                    <Trash2Icon className="size-3.5" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="top" className="bg-popover text-popover-foreground text-ui-sm">
                  {t("authzConfig.delete")}
                </TooltipContent>
              </Tooltip>
            </span>
          </div>
        ))}
      </ListSection>
    </ConfigDialog>
  );
}
