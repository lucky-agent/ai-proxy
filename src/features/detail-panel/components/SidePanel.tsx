import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Tooltip, TooltipTrigger, TooltipContent } from '@/components/ui/tooltip'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'

export type PanelTab = 'header' | 'query' | 'body' | 'raw' | 'form' | 'stream' | 'cookies' | 'console' | 'compare'

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
}: {
  title: string
  tab: PanelTab
  onTabChange: (tab: PanelTab) => void
  tabs: TabDef[]
  children: ReactNode
  onTitleClick?: () => void
}) {
  const { t } = useTranslation()

  return (
    <Tabs value={tab} onValueChange={(v) => onTabChange(v as PanelTab)} className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 items-center gap-0 border-b border-surface-elevated">
        {onTitleClick ? (
          <Tooltip>
            <TooltipTrigger className="inline-flex">
              <span
                onClick={onTitleClick}
                className="px-3 py-1.5 text-xs font-medium text-foreground cursor-pointer hover:bg-surface-elevated/50 rounded transition-colors"
              >
                {title}
              </span>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="bg-popover text-popover-foreground text-[11px]">
              Click to toggle full width
            </TooltipContent>
          </Tooltip>
        ) : (
          <span className="px-3 py-1.5 text-xs font-medium text-foreground">
            {title}
          </span>
        )}
        <TabsList variant="line" className="px-0 rounded-none bg-transparent h-auto">
          {tabs.map(x => (
            <TabsTrigger key={x.id} value={x.id} className="relative px-2.5 py-1.5 text-[11px]">
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
