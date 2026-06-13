import { useRef } from 'react'
import {
  LayoutSidebarOn,
  LayoutSidebarOff,
  LayoutBottomOn,
  LayoutBottomOff,
  LayoutRightOn,
  LayoutRightOff,
  ScriptIcon,
  ShieldIcon,
} from '@/components/icons'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'

export type DetailPosition = 'bottom' | 'right' | 'hidden'

interface BottomBarProps {
  showDomainSidebar: boolean
  onToggleDomainSidebar: () => void
  detailPosition: DetailPosition
  onToggleDetailPosition: (next: DetailPosition) => void
  scriptEnabled: boolean
  onToggleScript: () => void
  sslEnabled: boolean
  onToggleSsl: () => void
}

export function BottomBar({
  showDomainSidebar,
  onToggleDomainSidebar,
  detailPosition,
  onToggleDetailPosition,
  scriptEnabled,
  onToggleScript,
  sslEnabled,
  onToggleSsl,
}: BottomBarProps) {
  const { t } = useLocale()
  const clickTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const DetailIconOn = detailPosition === 'bottom' ? LayoutBottomOn : LayoutRightOn
  const DetailIconOff = detailPosition === 'bottom' ? LayoutBottomOff : LayoutRightOff
  const detailTitleKey = detailPosition === 'bottom' ? 'layout.detailBottom' : 'layout.detailRight'

  function handleDetailClick() {
    if (clickTimerRef.current) {
      clearTimeout(clickTimerRef.current)
      clickTimerRef.current = null
      onToggleDetailPosition('hidden')
    } else {
      clickTimerRef.current = setTimeout(() => {
        clickTimerRef.current = null
        onToggleDetailPosition(
          detailPosition === 'hidden' ? 'bottom'
            : detailPosition === 'bottom' ? 'right'
            : 'bottom'
        )
      }, 250)
    }
  }

  return (
    <div className="flex h-7 shrink-0 items-center border-t border-surface-elevated bg-surface-deep select-none px-2">
      {/* Left: host sidebar toggle */}
      <button
        type="button"
        onClick={onToggleDomainSidebar}
        className={cn(
          'relative inline-flex h-[22px] w-[26px] items-center justify-center rounded-md transition-colors',
          showDomainSidebar
            ? 'bg-surface-elevated text-foreground'
            : 'text-muted-foreground hover:bg-surface-elevated/50 hover:text-foreground'
        )}
        title={t('layout.hostSidebar')}>
        {showDomainSidebar && (
          <span className="absolute bottom-0 left-1 right-1 h-[2px] rounded-full bg-foreground/70" />
        )}
        {showDomainSidebar ? (
          <LayoutSidebarOn className="size-4" />
        ) : (
          <LayoutSidebarOff className="size-4" />
        )}
      </button>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Center-right: script toggle */}
      <button
        type="button"
        onClick={onToggleScript}
        className={cn(
          'relative inline-flex h-[22px] w-[26px] items-center justify-center rounded-md transition-colors',
          scriptEnabled
            ? 'bg-emerald-500/15 text-emerald-400'
            : 'text-muted-foreground hover:bg-surface-elevated/50 hover:text-foreground'
        )}
        title={t('scriptConfig.globalToggle')}>
        {scriptEnabled && (
          <span className="absolute bottom-0 left-1 right-1 h-[2px] rounded-full bg-emerald-400/70" />
        )}
        <ScriptIcon className="size-4" />
      </button>

      {/* Center-right: SSL toggle */}
      <button
        type="button"
        onClick={onToggleSsl}
        className={cn(
          'relative inline-flex h-[22px] w-[26px] items-center justify-center rounded-md transition-colors',
          sslEnabled
            ? 'bg-emerald-500/15 text-emerald-400'
            : 'text-muted-foreground hover:bg-surface-elevated/50 hover:text-foreground'
        )}
        title={t('sslConfig.globalToggle')}>
        {sslEnabled && (
          <span className="absolute bottom-0 left-1 right-1 h-[2px] rounded-full bg-emerald-400/70" />
        )}
        <ShieldIcon className="size-4" />
      </button>

      {/* Right: detail position toggle */}
      <button
        type="button"
        onClick={handleDetailClick}
        className={cn(
          'relative inline-flex h-[22px] w-[26px] items-center justify-center rounded-md transition-colors',
          detailPosition !== 'hidden'
            ? 'bg-surface-elevated text-foreground'
            : 'text-muted-foreground hover:bg-surface-elevated/50 hover:text-foreground'
        )}
        title={t(detailTitleKey)}>
        {detailPosition !== 'hidden' && (
          <span className="absolute bottom-0 left-1 right-1 h-[2px] rounded-full bg-foreground/70" />
        )}
        {detailPosition !== 'hidden' ? (
          <DetailIconOn className="size-4" />
        ) : (
          <DetailIconOff className="size-4" />
        )}
      </button>
    </div>
  )
}
