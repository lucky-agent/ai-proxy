import type { ReactNode } from "react";
import { HintIcon } from "@/components/core/HintIcon";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import { useLocale } from "@/hooks/useLocale";
import { cn } from "@/lib/utils";

interface ConfigDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  /** 标题旁问号里的说明 */
  description: string;
  /** 标题行总开关；不传 onEnabledChange 则不显示 */
  enabled?: boolean;
  onEnabledChange?: (enabled: boolean) => void;
  /** 标题下方的额外设置行（如 TLS 的「仅记录已解密流量」） */
  headerExtra?: ReactNode;
  /** 弹窗宽度类，如 `sm:max-w-md` */
  widthClass: string;
  error?: string;
  loading?: boolean;
  saving?: boolean;
  onSave: () => void;
  /** 侧滑编辑面板内容；传入即打开。打开时隐藏右上角关闭与底部按钮（保留占位） */
  panel?: ReactNode;
  /** 面板是否正在播放退场动画 */
  panelClosing?: boolean;
  /** 点击遮罩关闭面板 */
  onPanelDismiss?: () => void;
  /** 主内容（通常是 ListSection） */
  children: ReactNode;
}

/**
 * 配置类弹窗外壳：标题 + 问号说明 + 总开关、主内容、可选侧滑编辑面板、错误提示、取消 / 保存。
 * 侧滑面板状态用 `useSlidePanel` 管理。
 */
export function ConfigDialog({
  open,
  onOpenChange,
  title,
  description,
  enabled,
  onEnabledChange,
  headerExtra,
  widthClass,
  error,
  loading = false,
  saving = false,
  onSave,
  panel,
  panelClosing = false,
  onPanelDismiss,
  children,
}: ConfigDialogProps) {
  const { t } = useLocale();
  const panelOpen = panel != null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className={widthClass} showCloseButton={!panelOpen}>
        <DialogHeader>
          <div className="flex items-center gap-2.5">
            <DialogTitle>{title}</DialogTitle>
            <HintIcon label={description} />
            {onEnabledChange && <Switch checked={!!enabled} onCheckedChange={onEnabledChange} />}
          </div>
          {headerExtra}
        </DialogHeader>

        {/* 包裹内容区 + error + footer：侧滑面板的定位锚 */}
        <div className="relative grid gap-4">
          <div
            className={cn(
              "overflow-hidden transition-opacity duration-200",
              panelOpen && "pointer-events-none opacity-30",
            )}
          >
            {children}
          </div>

          {/* 侧滑遮罩 + 编辑面板：放在 overflow-hidden 外层，避免裁剪导致列表抖动 */}
          {panelOpen && (
            <>
              <div
                className={cn(
                  "absolute inset-0 z-10 bg-black/5",
                  panelClosing ? "animate-fade-out-overlay" : "animate-fade-in-overlay",
                )}
                onClick={onPanelDismiss}
              />
              <div
                className={cn(
                  "absolute inset-y-0 right-0 z-20 flex w-[min(72%,420px)] flex-col border-l border-border bg-popover shadow-lg",
                  panelClosing ? "animate-slide-out-right" : "animate-slide-in-right",
                )}
              >
                <div className="flex flex-1 flex-col overflow-y-auto p-4">{panel}</div>
              </div>
            </>
          )}

          {error && (
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}

          {/* 面板打开时仅隐藏但保留占位，防止弹窗高度收缩 */}
          <DialogFooter className={panelOpen ? "invisible" : ""}>
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              {t("common.cancel")}
            </Button>
            <Button variant="outline" onClick={onSave} disabled={loading || saving}>
              {saving ? t("common.saving") : t("common.save")}
            </Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
}
