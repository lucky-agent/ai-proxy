import { XIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import type { ViewId } from '@/types/view'

const VIEW_TABS: { id: ViewId; labelKey: string; closable?: boolean }[] = [
  { id: 'proxy', labelKey: 'view.proxy', closable: false },
  { id: 'new-request', labelKey: 'view.newRequest', closable: true },
  { id: 'ai', labelKey: 'view.ai', closable: true },
]

interface TabBarProps {
  activeView: ViewId
  mountedViews: Set<ViewId>
  onViewChange: (view: ViewId) => void
  onCloseTab: (view: ViewId) => void
}

function stopTitleBarDrag(event: { stopPropagation: () => void }) {
  event.stopPropagation()
}

export function TabBar({ activeView, mountedViews, onViewChange, onCloseTab }: TabBarProps) {
  const { t } = useLocale()

  // Only proxy mounted → hide the entire tab bar
  if (mountedViews.size <= 1) return null

  return (
    <div className="flex items-center gap-1" data-tauri-drag-region={false}>
      {VIEW_TABS.filter(({ id }) => mountedViews.has(id)).map(({ id, labelKey, closable }) => (
        <button
          key={id}
          type="button"
          data-tauri-drag-region={false}
          onMouseDown={stopTitleBarDrag}
          onPointerDown={stopTitleBarDrag}
          onClick={() => onViewChange(id)}
          className={cn(
            'inline-flex items-center gap-1 rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors',
            activeView === id
              ? 'bg-surface-elevated/50 text-foreground'
              : 'text-muted-foreground hover:text-foreground'
          )}>
          {t(labelKey)}
          {closable && (
            <span
              role="button"
              tabIndex={-1}
              data-tauri-drag-region={false}
              onMouseDown={stopTitleBarDrag}
              onPointerDown={stopTitleBarDrag}
              onClick={(e) => {
                e.stopPropagation()
                onCloseTab(id)
              }}
              className={cn(
                'inline-flex items-center justify-center rounded p-0.5 transition-colors',
                'text-muted-foreground/50 hover:text-muted-foreground hover:bg-surface-elevated/30'
              )}>
              <XIcon className="size-3" />
            </span>
          )}
        </button>
      ))}
    </div>
  )
}
