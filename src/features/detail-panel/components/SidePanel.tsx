import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Tooltip, TooltipTrigger, TooltipContent } from '@/components/ui/tooltip'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { formatBodySize } from '@/lib/format'

export type PanelTab = 'header' | 'query' | 'body' | 'raw' | 'form' | 'stream' | 'cookies' | 'console'

export interface TabDef {
  id: PanelTab
  labelKey: string
}

export default function SidePanel({
  title,
  tab,
  onTabChange,
  tabs,
  children,
  onTitleClick,
  bodySize,
}: {
  title: string
  tab: PanelTab
  onTabChange: (tab: PanelTab) => void
  tabs: TabDef[]
  children: ReactNode
  onTitleClick?: () => void
  /** body 字节数，用于标题右侧展示，如 "Response (2.4 KB)"。nil / 0 时不展示。 */
  bodySize?: number | null
}) {
  const { t } = useTranslation()
  const sizeLabel = formatBodySize(bodySize)

  return (
    <Tabs value={tab} onValueChange={(v) => onTabChange(v as PanelTab)} className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 items-center gap-0 border-b border-surface-elevated overflow-hidden">
        {onTitleClick ? (
          <Tooltip>
            <TooltipTrigger className="inline-flex">
              <span
                onClick={onTitleClick}
                className="px-3 py-1.5 text-xs font-medium text-foreground cursor-pointer hover:bg-surface-elevated/50 rounded transition-colors whitespace-nowrap shrink-0"
              >
                {title}
                {sizeLabel && <span className="ml-0.5 text-muted-foreground font-normal">({sizeLabel})</span>}
              </span>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="bg-popover text-popover-foreground text-ui-sm">
              Click to toggle full width
            </TooltipContent>
          </Tooltip>
        ) : (
          <span className="px-3 py-1.5 text-xs font-medium text-foreground whitespace-nowrap shrink-0">
            {title}
            {sizeLabel && <span className="ml-0.5 text-muted-foreground font-normal">({sizeLabel})</span>}
          </span>
        )}
        <TabsList variant="line" className="px-0 rounded-none bg-transparent h-auto min-w-0 overflow-x-auto overflow-y-hidden">
          {tabs.map(x => (
            <TabsTrigger key={x.id} value={x.id} className="relative px-2.5 py-1.5 text-ui-sm whitespace-nowrap">
              {t(x.labelKey)}
            </TabsTrigger>
          ))}
        </TabsList>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {children}
      </div>
    </Tabs>
  )
}
