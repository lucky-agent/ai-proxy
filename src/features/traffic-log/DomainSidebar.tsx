import { useTranslation } from 'react-i18next'
import { useState } from 'react'
import { ChevronRightIcon } from 'lucide-react'

interface Props {
  domains: [string, number][]
  totalEntries: number
  selectedDomain: string | null
  onSelectDomain: (domain: string | null) => void
}

export default function DomainSidebar({
  domains,
  totalEntries,
  selectedDomain,
  onSelectDomain,
}: Props) {
  const { t } = useTranslation()
  const [collapsed, setCollapsed] = useState(false)

  return (
    <div className="flex flex-col h-full border-r border-border overflow-hidden bg-background">
      <div className="px-3 py-2 border-b border-border text-[10px] font-semibold uppercase tracking-wide text-muted-foreground select-none">
        {t('hosts.title')}
      </div>
      <div className="flex-1 overflow-y-auto bg-background">
        <div
          className={`flex items-center gap-2 px-3 py-1.5 text-xs cursor-pointer transition-colors ${
            !selectedDomain
              ? 'bg-accent text-accent-foreground'
              : 'hover:bg-muted/50 text-foreground/80'
          }`}
          onClick={() => {
            onSelectDomain(null)
            setCollapsed(!collapsed)
          }}>
          <ChevronRightIcon
            className={`size-3 shrink-0 transition-transform ${collapsed ? '' : 'rotate-90'}`}
          />
          <span className="flex-1 truncate font-medium">{t('hosts.all')}</span>
          <span className="text-muted-foreground tabular-nums text-[10px]">{totalEntries}</span>
        </div>
        {!collapsed &&
          domains.map(([host, count]) => (
            <div
              key={host}
              className={`flex items-center gap-2 pl-6 pr-3 py-1.5 text-xs cursor-pointer transition-colors border-b border-border/30 ${
                selectedDomain === host
                  ? 'bg-accent text-accent-foreground'
                  : 'hover:bg-muted/50 text-foreground/80'
              }`}
              onClick={() => onSelectDomain(host)}>
              <span className="flex-1 truncate" title={host}>
                {host}
              </span>
              <span className="text-muted-foreground tabular-nums text-[10px]">{count}</span>
            </div>
          ))}
      </div>
    </div>
  )
}
