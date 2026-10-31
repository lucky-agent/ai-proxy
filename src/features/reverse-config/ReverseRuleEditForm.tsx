import { ArrowRightIcon, XIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useLocale } from "@/hooks/useLocale";
import { cn } from "@/lib/utils";
import type { ReverseProxyRule } from "@/types/settings";

interface Props {
  /** null = 新增；否则为被编辑规则（用于表单预填） */
  initial: ReverseProxyRule | null;
  onConfirm: (rule: Omit<ReverseProxyRule, "enabled">) => void;
  onCancel: () => void;
}

const FIELD_LABEL = "text-ui-xs font-medium tracking-wide text-muted-foreground";
const FIELD_INPUT = "h-8 rounded-md border-input px-2.5 text-xs transition-colors focus-visible:ring-1";

/** 纯表单组件 — 不含 Dialog 外壳，由父组件嵌入侧滑面板使用 */
export default function ReverseRuleEditForm({ initial, onConfirm, onCancel }: Props) {
  const { t } = useLocale();
  const [listenHost, setListenHost] = useState("127.0.0.1");
  const [listenPort, setListenPort] = useState(0);
  const [scheme, setScheme] = useState("https");
  const [upstreamHost, setUpstreamHost] = useState("");
  const [upstreamPort, setUpstreamPort] = useState(0);
  const [error, setError] = useState("");

  useEffect(() => {
    setListenHost(initial?.listen_host ?? "127.0.0.1");
    setListenPort(initial?.listen_port ?? 0);
    setScheme(initial?.upstream_scheme || "https");
    setUpstreamHost(initial?.upstream_host ?? "");
    setUpstreamPort(initial?.upstream_port ?? 0);
    setError("");
  }, [initial]);

  function clearError() {
    setError("");
  }

  function handleConfirm() {
    const host = upstreamHost.trim();
    if (!host) {
      setError(t("reverseConfig.hostRequired"));
      return;
    }
    if (listenPort < 1 || listenPort > 65535) {
      setError(t("reverseConfig.listenPortRequired"));
      return;
    }
    onConfirm({
      listen_host: listenHost.trim() || "127.0.0.1",
      listen_port: listenPort,
      upstream_scheme: scheme,
      upstream_host: host,
      upstream_port: upstreamPort,
    });
  }

  const titleKey = initial ? "reverseConfig.editRule" : "reverseConfig.addRule";

  return (
    <div className="flex h-full flex-col">
      {/* 标题栏：关闭按钮 + 标题 */}
      <div className="flex items-center gap-2 pb-4">
        <Button variant="ghost" size="icon-sm" onClick={onCancel}>
          <XIcon className="size-4" />
          <span className="sr-only">{t("reverseConfig.back")}</span>
        </Button>
        <h3 className="text-sm font-medium text-foreground">{t(titleKey)}</h3>
      </div>

      <div className="flex-1 space-y-4 overflow-y-auto">
        {/* 本地监听 */}
        <div className="grid gap-2">
          <span className={FIELD_LABEL}>{t("reverseConfig.local")}</span>
          <div className="grid grid-cols-[1fr,96px] gap-2">
            <div className="grid gap-1.5">
              <span className={FIELD_LABEL}>{t("reverseConfig.listenHost")}</span>
              <Input
                className={cn(FIELD_INPUT, "font-mono")}
                value={listenHost}
                placeholder="127.0.0.1"
                onChange={(e) => {
                  setListenHost(e.target.value);
                  clearError();
                }}
              />
            </div>
            <div className="grid gap-1.5">
              <span className={FIELD_LABEL}>{t("reverseConfig.listenPort")}</span>
              <Input
                className={FIELD_INPUT}
                type="number"
                min={1}
                max={65535}
                value={listenPort || ""}
                placeholder="8080"
                onChange={(e) => {
                  setListenPort(Number(e.target.value) || 0);
                  clearError();
                }}
              />
            </div>
          </div>
        </div>

        {/* 上游目标 */}
        <div className="grid gap-2">
          <span className={cn(FIELD_LABEL, "flex items-center gap-1")}>
            {t("reverseConfig.upstream")}
            <ArrowRightIcon className="size-3 text-muted-foreground/60" />
          </span>
          <div className="grid grid-cols-[88px,1fr,96px] gap-2">
            <div className="grid gap-1.5">
              <span className={FIELD_LABEL}>{t("reverseConfig.upstreamScheme")}</span>
              <Select value={scheme} onValueChange={(v) => v && setScheme(v)}>
                <SelectTrigger className={cn(FIELD_INPUT, "w-full")}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent className="min-w-24">
                  <SelectItem value="https">https</SelectItem>
                  <SelectItem value="http">http</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="grid gap-1.5">
              <span className={FIELD_LABEL}>{t("reverseConfig.upstreamHost")}</span>
              <Input
                className={cn(FIELD_INPUT, "font-mono")}
                value={upstreamHost}
                placeholder="api.example.com"
                onChange={(e) => {
                  setUpstreamHost(e.target.value);
                  clearError();
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleConfirm();
                }}
              />
            </div>
            <div className="grid gap-1.5">
              <span className={FIELD_LABEL}>{t("reverseConfig.upstreamPort")}</span>
              <Input
                className={FIELD_INPUT}
                type="number"
                min={1}
                max={65535}
                value={upstreamPort || ""}
                placeholder="443"
                onChange={(e) => {
                  setUpstreamPort(Number(e.target.value) || 0);
                  clearError();
                }}
              />
            </div>
          </div>
        </div>

        {error && <span className="text-ui-sm text-destructive">{error}</span>}
      </div>

      {/* 底部按钮 */}
      <div className="-mx-4 -mb-4 mt-4 flex items-center justify-end gap-2 rounded-b-xl border-t bg-muted/50 px-4 py-3">
        <Button variant="outline" size="sm" onClick={onCancel}>
          {t("reverseConfig.cancel")}
        </Button>
        <Button size="sm" onClick={handleConfirm}>
          {t("reverseConfig.confirm")}
        </Button>
      </div>
    </div>
  );
}
