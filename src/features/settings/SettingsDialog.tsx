import { Select as SelectPrimitive } from "@base-ui/react/select";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { DownloadIcon, GlobeIcon, MonitorIcon, MoonIcon, ServerIcon, ShieldCheckIcon, SunIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { HintIcon } from "@/components/core/HintIcon";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useLocale } from "@/hooks/useLocale";
import type { ProseFontSize } from "@/hooks/useProseFontSize";
import type { Theme } from "@/hooks/useTheme";
import type { LocaleSetting } from "@/i18n";
import { cn } from "@/lib/utils";
import type { ProxyConfig, UpstreamRouteDecision } from "@/types/settings";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
  proseFontSize: ProseFontSize;
  onProseFontSizeChange: (size: ProseFontSize) => void;
}

type SettingsTab = "general" | "proxy" | "certificate";

interface NavItem {
  value: SettingsTab;
  icon: typeof GlobeIcon;
  labelKey: string;
}

const NAV_ITEMS: NavItem[] = [
  { value: "general", icon: GlobeIcon, labelKey: "settings.generalTab" },
  { value: "proxy", icon: ServerIcon, labelKey: "settings.proxyTab" },
  { value: "certificate", icon: ShieldCheckIcon, labelKey: "settings.certificateTab" },
];

const inputClass = "h-auto w-full";
/** 端口列：窄一档，并隐藏 number 型上下箭头（见 index.css 的 .no-spinner） */
const portInputClass = "h-auto w-18 shrink-0 no-spinner";
/** 标签行固定行高，让监听/上游两行的输入框保持同一竖直节奏 */
const fieldLabelClass = "flex h-4 items-center text-xs font-medium text-muted-foreground";

/** 手动模式下地址/端口为空时填入的默认上游代理 */
const DEFAULT_UPSTREAM_HOST = "127.0.0.1";
const DEFAULT_UPSTREAM_PORT = 7890;

type UpstreamMode = "direct" | "manual" | "system";

/**
 * 多行文本 → 规则数组：先按行拆，再按 `,` / `;` 拆（兼容 NO_PROXY 与 Windows
 * 代理例外列表的写法），去首尾空白、丢弃空项。`#` 开头的注释行整行保留不拆，
 * 避免注释里的逗号把后半段变成一条规则（后端会跳过注释行）。
 */
function parseRuleLines(text: string): string[] {
  return text.split(/\r?\n/).flatMap((line) => {
    const l = line.trim();
    if (l === "") return [];
    if (l.startsWith("#")) return [l];
    return l
      .split(/[,;]/)
      .map((r) => r.trim())
      .filter((r) => r !== "");
  });
}

const ruleTextareaClass = "min-h-14 max-h-28 font-mono text-prose-sm md:text-prose-sm";
/** 设置区块卡片底色：在弹窗（--popover）之上叠一层淡 muted，明暗两套主题都只是轻微区分。
 *  不要用 surface-deep：暗色下它比 popover 暗很多，会变成黑块。 */
const cardClass = "rounded-lg border border-border bg-muted/40 p-3";
const sectionClass = `grid gap-3 ${cardClass}`;
const sectionTitleClass = "text-ui-md font-medium text-foreground";

/** 地址 + 端口一行，两列都带可见标签 */
function HostPortRow({
  hostLabel,
  portLabel,
  host,
  port,
  hostPlaceholder,
  onHostChange,
  onPortChange,
}: {
  hostLabel: string;
  portLabel: string;
  host: string;
  port: number;
  hostPlaceholder?: string;
  onHostChange: (v: string) => void;
  onPortChange: (v: number) => void;
}) {
  return (
    <div className="flex items-end gap-2">
      <label className="grid min-w-0 flex-1 gap-1.5">
        <span className={fieldLabelClass}>{hostLabel}</span>
        <Input
          className={inputClass}
          spellCheck={false}
          placeholder={hostPlaceholder}
          value={host}
          onChange={(e) => onHostChange(e.target.value)}
        />
      </label>
      <label className="grid shrink-0 gap-1.5">
        <span className={fieldLabelClass}>{portLabel}</span>
        <Input
          className={portInputClass}
          type="number"
          min={1}
          max={65535}
          value={port || ""}
          onChange={(e) => onPortChange(Number(e.target.value) || 0)}
        />
      </label>
    </div>
  );
}

export default function SettingsDialog({
  open,
  onOpenChange,
  theme,
  onThemeChange,
  proseFontSize,
  onProseFontSizeChange,
}: Props) {
  const { t, setLocale } = useLocale();
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [proxy, setProxy] = useState<ProxyConfig>({
    listen_host: "127.0.0.1",
    listen_port: 5201,
    upstream_proxy_enabled: false,
    upstream_proxy_host: "",
    upstream_proxy_port: 0,
    upstream_proxy_follow_system: false,
    upstream_proxy_include: [],
    upstream_proxy_exclude: [],
  });
  // 包含/排除规则以多行文本编辑（每行一条），保存时再拆成数组，避免编辑中途丢失空行
  const [includeText, setIncludeText] = useState("");
  const [excludeText, setExcludeText] = useState("");
  const [ruleTestUrl, setRuleTestUrl] = useState("");
  const [ruleTestResult, setRuleTestResult] = useState<UpstreamRouteDecision | null | undefined>(undefined);
  const [language, setLanguage] = useState<LocaleSetting>("system");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  // 证书安装/导出状态
  const [certInstalling, setCertInstalling] = useState(false);
  const [certMsg, setCertMsg] = useState("");

  // 上游代理三态：直连 / 手动 / 跟随系统，由两个后端 bool 派生。
  const upstreamMode: UpstreamMode = proxy.upstream_proxy_follow_system
    ? "system"
    : proxy.upstream_proxy_enabled
      ? "manual"
      : "direct";

  // 「手动」需要有效的地址与端口；未填或端口越界时提示，并阻止保存。
  const manualValid =
    proxy.upstream_proxy_host.trim() !== "" && proxy.upstream_proxy_port >= 1 && proxy.upstream_proxy_port <= 65535;

  function handleUpstreamModeChange(mode: UpstreamMode) {
    setProxy((p) => {
      // 切到「手动」且尚未填写时，补一个默认值 127.0.0.1:7890。
      const isEmpty = p.upstream_proxy_host.trim() === "" && p.upstream_proxy_port === 0;
      return {
        ...p,
        upstream_proxy_enabled: mode !== "direct",
        upstream_proxy_follow_system: mode === "system",
        ...(mode === "manual" && isEmpty
          ? { upstream_proxy_host: DEFAULT_UPSTREAM_HOST, upstream_proxy_port: DEFAULT_UPSTREAM_PORT }
          : {}),
      };
    });
  }

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    setError("");
    Promise.all([invoke<{ proxy: ProxyConfig }>("get_settings"), invoke<LocaleSetting>("get_locale")])
      .then(([settings, locale]) => {
        setProxy({
          ...settings.proxy,
          upstream_proxy_host: settings.proxy.upstream_proxy_host.trim(),
          upstream_proxy_port: settings.proxy.upstream_proxy_port,
        });
        setIncludeText((settings.proxy.upstream_proxy_include ?? []).join("\n"));
        setExcludeText((settings.proxy.upstream_proxy_exclude ?? []).join("\n"));
        setRuleTestUrl("");
        setLanguage(locale);
      })
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, [open]);

  // 规则测试：对当前草稿规则（未保存）实时判定，输入停顿 200ms 后调用后端。
  useEffect(() => {
    const url = ruleTestUrl.trim();
    if (url === "") {
      setRuleTestResult(undefined);
      return;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      invoke<UpstreamRouteDecision | null>("test_upstream_proxy_rules", {
        include: parseRuleLines(includeText),
        exclude: parseRuleLines(excludeText),
        url,
      })
        .then((r) => !cancelled && setRuleTestResult(r))
        .catch(() => !cancelled && setRuleTestResult(null));
    }, 200);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [ruleTestUrl, includeText, excludeText]);

  /** 测试结果 → 徽章（代理/直连）+ 原因说明；proxied=null 表示输入无法解析 */
  function ruleTestMessage(r: UpstreamRouteDecision | null): { detail: string; proxied: boolean | null } {
    if (r === null) return { detail: t("settings.upstreamRuleTestInvalid"), proxied: null };
    switch (r.route) {
      case "proxy":
        return {
          detail: r.rule
            ? t("settings.upstreamRuleTestIncludedBy", { rule: r.rule })
            : t("settings.upstreamRuleTestDefault"),
          proxied: true,
        };
      case "excluded":
        return { detail: t("settings.upstreamRuleTestExcludedBy", { rule: r.rule }), proxied: false };
      case "not_included":
        return { detail: t("settings.upstreamRuleTestNotIncluded"), proxied: false };
    }
  }

  async function handleSave() {
    if (upstreamMode === "manual" && !manualValid) {
      setError(t("settings.upstreamProxyManualInvalid"));
      return;
    }
    setSaving(true);
    setError("");
    try {
      await invoke("save_settings", {
        proxy: {
          ...proxy,
          upstream_proxy_include: parseRuleLines(includeText),
          upstream_proxy_exclude: parseRuleLines(excludeText),
        },
      });
      setLocale(language);
      onOpenChange(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleInstallCert() {
    setCertInstalling(true);
    setCertMsg("");
    try {
      const msg = await invoke<string>("install_ca_cert");
      setCertMsg(msg);
    } catch (err) {
      setCertMsg(String(err));
    } finally {
      setCertInstalling(false);
    }
  }

  async function handleExportCert() {
    setCertMsg("");
    try {
      const filePath = await save({
        defaultPath: "ai-proxy-ca-cert.pem",
        filters: [{ name: "Certificate", extensions: ["pem", "crt"] }],
      });
      if (!filePath) return; // 用户取消
      await invoke("export_ca_cert", { destPath: filePath });
      setCertMsg(`Certificate exported to ${filePath}`);
    } catch (err) {
      setCertMsg(String(err));
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <div className="flex items-center gap-2.5">
            <DialogTitle>{t("settings.title")}</DialogTitle>
            <HintIcon label={t("settings.description")} />
          </div>
        </DialogHeader>

        {loading ? (
          <p className="h-[min(28rem,65vh)] text-sm text-muted-foreground">{t("settings.loading")}</p>
        ) : (
          <Tabs
            value={activeTab}
            onValueChange={(v) => setActiveTab(v as SettingsTab)}
            orientation="vertical"
            className="flex h-[min(28rem,65vh)] min-w-0 flex-row"
          >
            <TabsList
              variant="line"
              className="h-auto w-[128px] shrink-0 flex-col items-start rounded-none border-r border-border py-2"
            >
              {NAV_ITEMS.map(({ value, icon: Icon, labelKey }) => (
                <TabsTrigger
                  key={value}
                  value={value}
                  className="w-full justify-start gap-2.5 px-3 py-2 text-left text-ui-lg font-medium"
                >
                  <Icon className="size-4 shrink-0" />
                  {t(labelKey)}
                </TabsTrigger>
              ))}
            </TabsList>

            <div className="min-h-0 min-w-0 flex-1 overflow-y-auto px-5 py-2">
              <TabsContent value="general" className="grid gap-4">
                <div className="grid gap-1.5">
                  <span className="text-xs font-medium text-muted-foreground">{t("settings.language")}</span>
                  <Select value={language} onValueChange={(v) => v && setLanguage(v as LocaleSetting)}>
                    <SelectTrigger className="w-full">
                      <SelectValue>
                        {(value: unknown) =>
                          value === "zh"
                            ? t("settings.languageZh")
                            : value === "en"
                              ? t("settings.languageEn")
                              : t("settings.languageSystem")
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectPrimitive.Portal>
                      <SelectPrimitive.Positioner
                        side="bottom"
                        sideOffset={4}
                        alignItemWithTrigger={false}
                        collisionAvoidance={{ side: "none" }}
                        className="isolate z-50"
                      >
                        <SelectPrimitive.Popup className="relative isolate z-50 max-h-(--available-height) w-(--anchor-width) min-w-36 origin-(--transform-origin) overflow-x-hidden overflow-y-auto rounded-lg bg-popover text-popover-foreground shadow-md ring-1 ring-foreground/10 duration-100 data-[side=bottom]:slide-in-from-top-2 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95">
                          <SelectPrimitive.List>
                            <SelectItem value="system">{t("settings.languageSystem")}</SelectItem>
                            <SelectItem value="en">{t("settings.languageEn")}</SelectItem>
                            <SelectItem value="zh">{t("settings.languageZh")}</SelectItem>
                          </SelectPrimitive.List>
                        </SelectPrimitive.Popup>
                      </SelectPrimitive.Positioner>
                    </SelectPrimitive.Portal>
                  </Select>
                </div>

                <div className="grid gap-1.5">
                  <span className="text-xs font-medium text-muted-foreground">{t("settings.theme")}</span>
                  <ToggleGroup
                    value={[theme]}
                    onValueChange={(value) => {
                      if (value && value.length > 0) onThemeChange(value[0] as Theme);
                    }}
                    size="sm"
                    className="w-fit rounded-md border border-border bg-surface-elevated/50 p-0.5"
                  >
                    {(
                      [
                        { value: "light" as Theme, icon: SunIcon },
                        { value: "dark" as Theme, icon: MoonIcon },
                        { value: "system" as Theme, icon: MonitorIcon },
                      ] as const
                    ).map(({ value, icon: Icon }) => (
                      <Tooltip key={value}>
                        <TooltipTrigger className="inline-flex">
                          <ToggleGroupItem
                            value={value}
                            className="inline-flex size-7 items-center justify-center rounded-[4px] border border-transparent text-muted-foreground transition-colors cursor-pointer hover:text-foreground aria-pressed:border-ring/60 aria-pressed:bg-background aria-pressed:text-foreground aria-pressed:shadow-sm aria-pressed:hover:bg-background"
                          >
                            <Icon className="size-3.5" />
                          </ToggleGroupItem>
                        </TooltipTrigger>
                        <TooltipContent side="top" className="bg-popover text-popover-foreground text-ui-sm">
                          {t(`theme.${value}`)}
                        </TooltipContent>
                      </Tooltip>
                    ))}
                  </ToggleGroup>
                </div>

                <div className="grid gap-1.5">
                  <div className="flex items-center gap-1.5">
                    <span className="text-xs font-medium text-muted-foreground">{t("settings.proseFontSize")}</span>
                    <HintIcon label={t("settings.proseFontSizeHint")} />
                  </div>
                  <ToggleGroup
                    value={[proseFontSize]}
                    onValueChange={(value) => {
                      if (value && value.length > 0) onProseFontSizeChange(value[0] as ProseFontSize);
                    }}
                    size="sm"
                    className="w-fit rounded-md border border-border bg-surface-elevated/50 p-0.5"
                  >
                    {(
                      [
                        { value: "small" as ProseFontSize, labelKey: "settings.proseFontSizeSmall" },
                        { value: "normal" as ProseFontSize, labelKey: "settings.proseFontSizeNormal" },
                        { value: "large" as ProseFontSize, labelKey: "settings.proseFontSizeLarge" },
                      ] as const
                    ).map(({ value, labelKey }) => (
                      <ToggleGroupItem
                        key={value}
                        value={value}
                        className="inline-flex h-7 items-center justify-center rounded-[4px] border border-transparent px-2.5 text-ui-md text-muted-foreground transition-colors cursor-pointer hover:text-foreground aria-pressed:border-ring/60 aria-pressed:bg-background aria-pressed:text-foreground aria-pressed:shadow-sm aria-pressed:hover:bg-background"
                      >
                        {t(labelKey)}
                      </ToggleGroupItem>
                    ))}
                  </ToggleGroup>
                </div>
              </TabsContent>

              <TabsContent value="proxy" className="grid content-start gap-3">
                {/* ── 本地监听 ── */}
                <section className={sectionClass}>
                  <span className={sectionTitleClass}>{t("settings.listenSection")}</span>
                  <HostPortRow
                    hostLabel={t("settings.hostLabel")}
                    portLabel={t("settings.portLabel")}
                    host={proxy.listen_host}
                    port={proxy.listen_port}
                    onHostChange={(v) => setProxy((p) => ({ ...p, listen_host: v }))}
                    onPortChange={(v) => setProxy((p) => ({ ...p, listen_port: v }))}
                  />
                </section>

                {/* ── 上游代理 ── */}
                <section className={sectionClass}>
                  <div className="flex items-center gap-1.5">
                    <span className={sectionTitleClass}>{t("settings.upstreamProxyHost")}</span>
                    <HintIcon label={t("settings.upstreamProxyHint")} />
                  </div>
                  <ToggleGroup
                    value={[upstreamMode]}
                    onValueChange={(value) => {
                      if (value && value.length > 0) handleUpstreamModeChange(value[0] as UpstreamMode);
                    }}
                    size="sm"
                    className="w-fit rounded-md border border-border bg-surface-elevated/50 p-0.5"
                  >
                    {(
                      [
                        { value: "direct" as UpstreamMode, labelKey: "settings.upstreamModeDirect" },
                        { value: "manual" as UpstreamMode, labelKey: "settings.upstreamModeManual" },
                        { value: "system" as UpstreamMode, labelKey: "settings.upstreamModeSystem" },
                      ] as const
                    ).map(({ value, labelKey }) => (
                      <ToggleGroupItem
                        key={value}
                        value={value}
                        className="inline-flex h-7 items-center justify-center rounded-[4px] border border-transparent px-2.5 text-ui-md text-muted-foreground transition-colors cursor-pointer hover:text-foreground aria-pressed:border-ring/60 aria-pressed:bg-background aria-pressed:text-foreground aria-pressed:shadow-sm aria-pressed:hover:bg-background"
                      >
                        {t(labelKey)}
                      </ToggleGroupItem>
                    ))}
                  </ToggleGroup>
                  {upstreamMode === "manual" && (
                    <div className="grid gap-1.5">
                      <HostPortRow
                        hostLabel={t("settings.hostLabel")}
                        portLabel={t("settings.portLabel")}
                        host={proxy.upstream_proxy_host}
                        port={proxy.upstream_proxy_port}
                        hostPlaceholder={t("settings.upstreamProxyHostPlaceholder")}
                        onHostChange={(v) => setProxy((p) => ({ ...p, upstream_proxy_host: v }))}
                        onPortChange={(v) => setProxy((p) => ({ ...p, upstream_proxy_port: v }))}
                      />
                      {!manualValid && (
                        <p className="text-ui-sm text-destructive">{t("settings.upstreamProxyManualInvalid")}</p>
                      )}
                    </div>
                  )}

                  {upstreamMode !== "direct" && (
                    <div className="grid gap-2.5 border-t border-border pt-3">
                      <div className="flex items-center gap-1.5">
                        <span className={sectionTitleClass}>{t("settings.upstreamRules")}</span>
                        <HintIcon label={t("settings.upstreamRulesHint")} />
                      </div>

                      <label className="grid gap-1.5">
                        <span className={fieldLabelClass}>
                          {t("settings.upstreamProxyInclude")}
                          <span className="ml-1.5 font-normal text-muted-foreground/70">
                            {t("settings.upstreamProxyIncludeNote")}
                          </span>
                        </span>
                        <Textarea
                          className={ruleTextareaClass}
                          spellCheck={false}
                          placeholder="*.openai.com"
                          value={includeText}
                          onChange={(e) => setIncludeText(e.target.value)}
                        />
                      </label>

                      <label className="grid gap-1.5">
                        <span className={fieldLabelClass}>
                          {t("settings.upstreamProxyExclude")}
                          <span className="ml-1.5 font-normal text-muted-foreground/70">
                            {t("settings.upstreamProxyExcludeNote")}
                          </span>
                        </span>
                        <Textarea
                          className={ruleTextareaClass}
                          spellCheck={false}
                          placeholder={"localhost\n10.0.0.0/8"}
                          value={excludeText}
                          onChange={(e) => setExcludeText(e.target.value)}
                        />
                      </label>

                      <label className="grid gap-1.5">
                        <span className={fieldLabelClass}>{t("settings.upstreamRuleTest")}</span>
                        <Input
                          className={inputClass}
                          spellCheck={false}
                          placeholder={t("settings.upstreamRuleTestPlaceholder")}
                          value={ruleTestUrl}
                          onChange={(e) => setRuleTestUrl(e.target.value)}
                        />
                      </label>
                      {ruleTestResult !== undefined &&
                        (() => {
                          const { detail, proxied } = ruleTestMessage(ruleTestResult);
                          if (proxied === null) {
                            return <p className="text-ui-sm text-destructive">{detail}</p>;
                          }
                          return (
                            <div className="flex min-w-0 items-center gap-2 text-ui-sm">
                              <span
                                className={cn(
                                  "shrink-0 rounded px-1.5 py-0.5 text-ui-xs font-medium",
                                  proxied
                                    ? "bg-foreground text-background"
                                    : "border border-border bg-background text-foreground",
                                )}
                              >
                                {proxied ? t("settings.upstreamRouteProxy") : t("settings.upstreamRouteDirect")}
                              </span>
                              <span className="min-w-0 truncate text-muted-foreground" title={detail}>
                                {detail}
                              </span>
                            </div>
                          );
                        })()}
                    </div>
                  )}
                </section>
              </TabsContent>

              <TabsContent value="certificate" className="grid gap-4">
                <div className="flex items-center gap-1.5">
                  <span className="text-xs font-medium text-muted-foreground">{t("settings.certificateTab")}</span>
                  <HintIcon label={t("settings.certHint")} />
                </div>

                {/* 安装证书 */}
                <div className={cardClass}>
                  <div className="flex items-start justify-between gap-4">
                    <div className="grid gap-0.5">
                      <span className="text-ui-md font-medium text-foreground">{t("settings.certInstallTitle")}</span>
                      <span className="text-ui-sm text-muted-foreground">{t("settings.certInstallDesc")}</span>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      className="shrink-0"
                      onClick={handleInstallCert}
                      disabled={certInstalling}
                    >
                      <ShieldCheckIcon className="size-3.5" />
                      {certInstalling ? t("settings.saving") : t("settings.installCert")}
                    </Button>
                  </div>
                </div>

                {/* 导出证书 */}
                <div className={cardClass}>
                  <div className="flex items-start justify-between gap-4">
                    <div className="grid gap-0.5">
                      <span className="text-ui-md font-medium text-foreground">{t("settings.certExportTitle")}</span>
                      <span className="text-ui-sm text-muted-foreground">{t("settings.certExportDesc")}</span>
                    </div>
                    <Button variant="outline" size="sm" className="shrink-0" onClick={handleExportCert}>
                      <DownloadIcon className="size-3.5" />
                      {t("settings.exportCert")}
                    </Button>
                  </div>
                </div>

                {certMsg && (
                  <Alert
                    variant={certMsg.includes("already") || certMsg.includes("installed") ? "default" : "destructive"}
                  >
                    <AlertDescription>{certMsg}</AlertDescription>
                  </Alert>
                )}
              </TabsContent>
            </div>
          </Tabs>
        )}

        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("settings.cancel")}
          </Button>
          <Button variant="outline" onClick={handleSave} disabled={loading || saving}>
            {saving ? t("settings.saving") : t("settings.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
