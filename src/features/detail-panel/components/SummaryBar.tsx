import { CheckIcon, CopyIcon, XIcon } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useCopyToClipboard } from '@/hooks/useCopyToClipboard'
import { statusCategory, formatDuration } from '@/lib/format'
import type { TrafficEntry } from '@/types/proxy'
import { Badge } from '@/components/ui/badge'

export default function SummaryBar({ entry, onClose }: { entry: TrafficEntry; onClose?: () => void }) {
  const { t } = useTranslation()
  const { copied, copy } = useCopyToClipboard()

  return (
    <div className="flex shrink-0 items-center gap-2 border-b border-surface-elevated px-3 py-1.5 text-xs">
      <Badge
        className="shrink-0 rounded font-semibold uppercase"
        style={{
          color: `var(--badge-${entry.method.toLowerCase()})`,
          background: `color-mix(in oklch, var(--badge-${entry.method.toLowerCase()}) 10%, transparent)`,
          borderColor: `color-mix(in oklch, var(--badge-${entry.method.toLowerCase()}) 20%, transparent)`,
        }}>
        {entry.method}
      </Badge>
      <Badge
        className="shrink-0 rounded font-semibold"
        style={{
          color: `var(--badge-${statusCategory(entry.status ?? 0)})`,
        }}>
        {entry.status != null && (
          <span
            className="inline-block size-1.5 rounded-full bg-current"
          />
        )}
        {entry.status ?? t('detail.pending')}
      </Badge>
      <span className="min-w-0 flex-1 truncate text-primary" title={entry.uri}>
        {entry.uri}
      </span>
      {onClose && (
        <button
          onClick={onClose}
          className="shrink-0 rounded p-1 text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50 transition-colors"
          title="关闭详情">
          <XIcon className="size-3" />
        </button>
      )}
      <button
        onClick={() => copy(entry.uri)}
        className="shrink-0 rounded p-1 text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50 transition-colors"
        title={copied ? t('detail.copied') : t('detail.copyUri')}>
        {copied ? <CheckIcon className="size-3 text-primary" /> : <CopyIcon className="size-3" />}
      </button>
      {entry.durationMs != null && (
        <span className="shrink-0 text-muted-foreground tabular-nums">
          {formatDuration(entry.durationMs)}
        </span>
      )}
    </div>
  )
}
