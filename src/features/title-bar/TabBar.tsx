import { XIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import type { ViewId } from '@/types/view'

const VIEW_TABS: { id: ViewId; labelKey: string }[] = [
  { id: 'proxy', labelKey: 'view.proxy' },
  { id: 'new-request', labelKey: 'view.newRequest' },
  { id: 'ai', labelKey: 'view.ai' },
]

interface TabBarProps {
  activeView: ViewId
  onViewChange: (view: ViewId) => void
  onCloseTab: (view: ViewId) => void
}

function stopTitleBarDrag(event: { stopPropagation: () => void }) {
  event.stopPropagation()
}

export function TabBar({ activeView, onViewChange, onCloseTab }: TabBarProps) {
  const { t } = useLocale()

  return (
    <div className="flex items-center gap-1" data-tauri-drag-region={false}>
      {VIEW_TABS.map(({ id, labelKey }) => (
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
              ? 'bg-primary/10 text-primary'
              : 'text-muted-foreground hover:text-foreground'
          )}>
          {t(labelKey)}
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
        </button>
      ))}
    </div>
  )
}
