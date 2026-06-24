// src/features/new-request/RequestTabBar.tsx
import { useState, useRef, useEffect, useCallback } from 'react'
import { PlusIcon, XIcon, ChevronDownIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import type { RequestTab } from '@/types/collection'

interface RequestTabBarProps {
  tabs: RequestTab[]
  activeTabId: string | null
  onActivate: (tabId: string) => void
  onClose: (tabId: string) => void
  onNew: () => void
  onCloseOthers: () => void
  onCloseAll: () => void
}

export default function RequestTabBar({
  tabs,
  activeTabId,
  onActivate,
  onClose,
  onNew,
  onCloseOthers,
  onCloseAll,
}: RequestTabBarProps) {
  const { t } = useLocale()
  const containerRef = useRef<HTMLDivElement>(null)
  const [overflowIds, setOverflowIds] = useState<string[]>([])

  // 检测溢出：基于实际像素宽度和容器宽度做判断
  const detectOverflow = useCallback(() => {
    const el = containerRef.current
    if (!el) return

    const tabList = el.querySelector('[data-tab-list]') as HTMLElement | null
    if (!tabList) return

    // 右侧操作区宽度（溢出菜单按钮 + 新建按钮）
    const actionsWidth = 56 // approximate: 2 × icon buttons + spacing
    const availableWidth = el.clientWidth - actionsWidth

    // 收集所有 tab 元素及其宽度
    const tabEls = tabList.querySelectorAll<HTMLElement>('button[type="button"]')
    const tabEntries: { id: string; width: number }[] = []
    for (const tabEl of tabEls) {
      const tabId = tabs.find(t => {
        // match by method + name inside the button
        const methodSpan = tabEl.querySelector('span.font-semibold')
        return methodSpan?.textContent === t.method
      })
      if (tabId) {
        tabEntries.push({ id: tabId.id, width: tabEl.offsetWidth })
      }
    }

    // 如果 tab 元素和 tabs 数量不一致（比如刚 render 完还没同步），用简单策略回退
    if (tabEntries.length !== tabs.length && tabs.length > 0) {
      // approximate: each tab ~140px
      const avgTabWidth = 140
      const visibleCount = Math.floor(availableWidth / avgTabWidth)
      if (visibleCount < tabs.length) {
        setOverflowIds(tabs.slice(Math.max(0, visibleCount - 1)).map(t => t.id))
      } else {
        setOverflowIds([])
      }
      return
    }

    // 从右往左累计宽度，超出 availableWidth 的进溢出
    let cumulative = 0
    const overflow: string[] = []
    for (let i = tabEntries.length - 1; i >= 0; i--) {
      cumulative += tabEntries[i].width
      if (cumulative > availableWidth) {
        overflow.push(tabEntries[i].id)
      }
    }
    setOverflowIds(overflow)
  }, [tabs])

  useEffect(() => {
    detectOverflow()
  }, [detectOverflow, tabs.length])

  // 监听容器 resize
  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const ro = new ResizeObserver(detectOverflow)
    ro.observe(el)
    return () => ro.disconnect()
  }, [detectOverflow])

  // 区分可见 tab 和溢出 tab
  const visibleTabs = tabs.filter(t => !overflowIds.includes(t.id))
  const overflowTabs = tabs.filter(t => overflowIds.includes(t.id))

  const handleClose = (e: React.MouseEvent, tabId: string) => {
    e.stopPropagation()
    onClose(tabId)
  }

  const displayName = (tab: RequestTab) => tab.name || t('tab.untitled')

  return (
    <div
      ref={containerRef}
      className="flex shrink-0 items-center border-b border-border bg-surface-base/50"
    >
      {/* Tab 标签列表 */}
      <div data-tab-list className="flex flex-1 items-center overflow-hidden min-w-0">
        {visibleTabs.map(tab => {
          const isActive = tab.id === activeTabId
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => onActivate(tab.id)}
              className={cn(
                'group relative flex shrink-0 items-center gap-1.5 h-8 max-w-[160px] px-3',
                'text-xs border-r border-border cursor-pointer select-none',
                'hover:bg-surface-elevated/50 transition-colors',
                isActive && 'bg-surface-elevated text-accent',
                isActive && 'after:absolute after:bottom-0 after:left-0 after:right-0 after:h-[2px] after:bg-accent',
              )}
            >
              <span
                className={cn(
                  'font-semibold shrink-0',
                  tab.method === 'GET' && 'text-badge-get',
                  tab.method === 'POST' && 'text-badge-post',
                  tab.method === 'PUT' && 'text-badge-put',
                  tab.method === 'PATCH' && 'text-badge-patch',
                  tab.method === 'DELETE' && 'text-badge-delete',
                  tab.method === 'HEAD' && 'text-badge-head',
                  tab.method === 'OPTIONS' && 'text-badge-options',
                )}
              >
                {tab.method}
              </span>
              <span className="truncate">{displayName(tab)}</span>
              <button
                type="button"
                onClick={e => handleClose(e, tab.id)}
                className="shrink-0 ml-0.5 size-3.5 flex items-center justify-center rounded-sm
                           opacity-0 group-hover:opacity-100 hover:bg-border/50 transition-opacity"
                aria-label="Close"
              >
                <XIcon className="size-2.5" />
              </button>
            </button>
          )
        })}
      </div>

      {/* 右侧操作区 */}
      <div className="flex shrink-0 items-center">
        {/* 溢出菜单 */}
        {(overflowTabs.length > 0 || tabs.length > 0) && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon-sm" className="h-7 w-7">
                <ChevronDownIcon className="size-3.5" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="min-w-[160px]">
              {overflowTabs.map(tab => (
                <DropdownMenuItem
                  key={tab.id}
                  onClick={() => onActivate(tab.id)}
                >
                  <span className={cn(
                    'font-semibold text-[11px]',
                    tab.method === 'GET' && 'text-badge-get',
                    tab.method === 'POST' && 'text-badge-post',
                    tab.method === 'PUT' && 'text-badge-put',
                    tab.method === 'PATCH' && 'text-badge-patch',
                    tab.method === 'DELETE' && 'text-badge-delete',
                    tab.method === 'HEAD' && 'text-badge-head',
                    tab.method === 'OPTIONS' && 'text-badge-options',
                  )}>
                    {tab.method}
                  </span>
                  <span className="ml-1.5 truncate text-xs">{displayName(tab)}</span>
                </DropdownMenuItem>
              ))}
              {overflowTabs.length > 0 && <DropdownMenuSeparator />}
              <DropdownMenuItem onClick={onCloseOthers}>
                {t('tab.closeOthers')}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={onCloseAll}>
                {t('tab.closeAll')}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        )}

        {/* [+] 按钮 */}
        <Button
          variant="ghost"
          size="icon-sm"
          className="h-7 w-7 ml-0.5"
          onClick={onNew}
          aria-label={t('tab.newRequest')}
        >
          <PlusIcon className="size-3.5" />
        </Button>
      </div>
    </div>
  )
}
