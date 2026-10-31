import { PlusIcon } from "lucide-react";
import type { ReactNode, Ref } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

/**
 * 列表区固定高度：常规窗口下 20rem，窗口变矮时按 45vh 收缩，
 * 保证「标题 + 列表 + 底部按钮」整体不超出窗口（弹窗其余部分约 200px）。
 */
const LIST_HEIGHT = "h-[min(20rem,45vh)]";

interface ListSectionProps {
  /** 区块标题，如「规则列表」 */
  title: string;
  /** 计数文案（已本地化），列表为空时不显示 */
  countLabel?: string;
  /** 「添加」按钮文案与回调；不传则不显示按钮 */
  addLabel?: string;
  onAdd?: () => void;
  loading?: boolean;
  loadingLabel?: string;
  /** 列表为空时在固定高度区域内居中显示的提示 */
  empty: boolean;
  emptyLabel: string;
  /** 整块禁用（如总开关关闭）：变暗并禁止交互 */
  disabled?: boolean;
  /** 滚动容器 ref，用于滚动到命中行等 */
  listRef?: Ref<HTMLDivElement>;
  className?: string;
  children?: ReactNode;
}

/**
 * 配置弹窗里的列表区块：标题 + 计数 + 添加按钮，下方是**固定高度**的滚动区。
 * 加载中 / 空列表 / 有数据 三种状态都占同样高度，弹窗不会随条数或状态忽高忽低。
 */
export function ListSection({
  title,
  countLabel,
  addLabel,
  onAdd,
  loading = false,
  loadingLabel,
  empty,
  emptyLabel,
  disabled = false,
  listRef,
  className,
  children,
}: ListSectionProps) {
  return (
    <div
      className={cn(
        "grid gap-2 transition-opacity duration-200",
        // 加载中不变暗：此时只显示加载提示，还不知道总开关状态
        disabled && !loading && "pointer-events-none opacity-40",
        className,
      )}
    >
      {/* 加载中标题行隐藏但保留占位，整块高度与加载完成后一致 */}
      <div className={cn("flex items-center justify-between", loading && "invisible")} aria-hidden={loading}>
        <span className="text-xs font-medium text-muted-foreground">
          {title}
          {!empty && countLabel && <span className="ml-1 font-normal opacity-80">{countLabel}</span>}
        </span>
        {addLabel && onAdd && (
          <Button variant="ghost" size="xs" onClick={onAdd} tabIndex={loading ? -1 : undefined}>
            <PlusIcon className="size-3.5" />
            {addLabel}
          </Button>
        )}
      </div>

      {loading ? (
        <div className={cn(LIST_HEIGHT, "flex items-center justify-center text-sm text-muted-foreground")}>
          {loadingLabel}
        </div>
      ) : empty ? (
        <div
          className={cn(
            LIST_HEIGHT,
            "flex items-center justify-center rounded-[10px] border border-dashed border-border px-4 text-center text-xs text-muted-foreground",
          )}
        >
          {emptyLabel}
        </div>
      ) : (
        <div ref={listRef} className={cn(LIST_HEIGHT, "overflow-y-auto rounded-[10px] border border-border")}>
          {children}
        </div>
      )}
    </div>
  );
}
