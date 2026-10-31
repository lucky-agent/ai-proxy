import { XIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useLocale } from "@/hooks/useLocale";
import { cn } from "@/lib/utils";
import type { AuthzRule } from "@/types/settings";

interface Props {
  /** null = 新增；否则为被编辑规则（用于表单预填） */
  initial: AuthzRule | null;
  onConfirm: (rule: Omit<AuthzRule, "enabled">) => void;
  onCancel: () => void;
}

export const METHOD_ANY = "ANY";
export const METHOD_OPTIONS = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] as const;
export const DEFAULT_TIMEOUT = 500;

const FIELD_LABEL = "text-ui-xs font-medium tracking-wide text-muted-foreground";
const FIELD_INPUT = "h-8 rounded-md border-input px-2.5 text-xs transition-colors focus-visible:ring-1";

/** 纯表单组件 — 不含 Dialog 外壳，由父组件嵌入侧滑面板使用（与反向代理规则表单同构） */
export default function AuthzRuleEditForm({ initial, onConfirm, onCancel }: Props) {
  const { t } = useLocale();
  const [name, setName] = useState("");
  const [domain, setDomain] = useState("");
  const [method, setMethod] = useState("");
  const [url, setUrl] = useState("");
  const [timeoutMs, setTimeoutMs] = useState(DEFAULT_TIMEOUT);
  const [onError, setOnError] = useState<AuthzRule["on_error"]>("closed");
  const [error, setError] = useState("");

  useEffect(() => {
    setName(initial?.name ?? "");
    setDomain(initial?.domain ?? "");
    setMethod(initial?.method ?? "");
    setUrl(initial?.url ?? "");
    setTimeoutMs(initial?.timeout_ms || DEFAULT_TIMEOUT);
    setOnError(initial?.on_error ?? "closed");
    setError("");
  }, [initial]);

  function handleConfirm() {
    const trimmedName = name.trim();
    const trimmedUrl = url.trim();
    if (!trimmedName) {
      setError(t("authzConfig.nameRequired"));
      return;
    }
    if (!trimmedUrl) {
      setError(t("authzConfig.urlRequired"));
      return;
    }
    onConfirm({
      name: trimmedName,
      domain: domain.trim(),
      method,
      url: trimmedUrl,
      timeout_ms: timeoutMs,
      on_error: onError,
    });
  }

  /** 输入变化时清掉校验提示 */
  function edit<T>(setter: (v: T) => void) {
    return (v: T) => {
      setter(v);
      setError("");
    };
  }

  const submitOnEnter = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleConfirm();
  };

  const titleKey = initial ? "authzConfig.editRule" : "authzConfig.addRule";

  return (
    <div className="flex h-full flex-col">
      {/* 标题栏：关闭按钮 + 标题 */}
      <div className="flex items-center gap-2 pb-4">
        <Button variant="ghost" size="icon-sm" onClick={onCancel}>
          <XIcon className="size-4" />
          <span className="sr-only">{t("authzConfig.back")}</span>
        </Button>
        <h3 className="text-sm font-medium text-foreground">{t(titleKey)}</h3>
      </div>

      <div className="flex-1 space-y-4 overflow-y-auto">
        <div className="grid gap-1.5">
          <span className={FIELD_LABEL}>{t("authzConfig.name")}</span>
          <Input
            className={FIELD_INPUT}
            value={name}
            placeholder={t("authzConfig.placeholderName")}
            autoFocus
            onChange={(e) => edit(setName)(e.target.value)}
            onKeyDown={submitOnEnter}
          />
        </div>

        <div className="grid grid-cols-[1fr_96px] gap-2">
          <div className="grid gap-1.5">
            <span className={FIELD_LABEL}>{t("authzConfig.domain")}</span>
            <Input
              className={cn(FIELD_INPUT, "font-mono")}
              value={domain}
              placeholder={t("authzConfig.placeholderDomain")}
              onChange={(e) => edit(setDomain)(e.target.value)}
              onKeyDown={submitOnEnter}
            />
          </div>
          <div className="grid gap-1.5">
            <span className={FIELD_LABEL}>{t("authzConfig.method")}</span>
            <Select
              value={method || METHOD_ANY}
              onValueChange={(v) => edit(setMethod)(v === METHOD_ANY ? "" : String(v))}
            >
              <SelectTrigger className={cn(FIELD_INPUT, "w-full", !method && "text-muted-foreground")}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent className="min-w-24">
                <SelectItem value={METHOD_ANY}>Any</SelectItem>
                {METHOD_OPTIONS.map((m) => (
                  <SelectItem key={m} value={m}>
                    {m}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        <div className="grid gap-1.5">
          <span className={FIELD_LABEL}>{t("authzConfig.url")}</span>
          <Input
            className={cn(FIELD_INPUT, "font-mono")}
            value={url}
            placeholder={t("authzConfig.placeholderUrl")}
            onChange={(e) => edit(setUrl)(e.target.value)}
            onKeyDown={submitOnEnter}
          />
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div className="grid gap-1.5">
            <span className={FIELD_LABEL}>{t("authzConfig.timeout")}</span>
            <Input
              className={FIELD_INPUT}
              type="number"
              min={1}
              value={timeoutMs}
              onChange={(e) => edit(setTimeoutMs)(Number(e.target.value) || DEFAULT_TIMEOUT)}
            />
          </div>
          <div className="grid gap-1.5">
            <span className={FIELD_LABEL}>{t("authzConfig.onError")}</span>
            <Select value={onError} onValueChange={(v) => v && edit(setOnError)(v as AuthzRule["on_error"])}>
              <SelectTrigger className={cn(FIELD_INPUT, "w-full")}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent className="min-w-24">
                <SelectItem value="closed">{t("authzConfig.onErrorClosed")}</SelectItem>
                <SelectItem value="open">{t("authzConfig.onErrorOpen")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        {error && <span className="text-ui-sm text-destructive">{error}</span>}
      </div>

      {/* 底部按钮 */}
      <div className="-mx-4 -mb-4 mt-4 flex items-center justify-end gap-2 rounded-b-xl border-t bg-muted/50 px-4 py-3">
        <Button variant="outline" size="sm" onClick={onCancel}>
          {t("authzConfig.cancel")}
        </Button>
        <Button size="sm" onClick={handleConfirm}>
          {t("authzConfig.confirm")}
        </Button>
      </div>
    </div>
  );
}
