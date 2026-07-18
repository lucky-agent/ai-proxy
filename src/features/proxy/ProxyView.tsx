import { type TypeFilter } from '@/lib/format'
import { TrafficLog } from './traffic-log'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { TypeFilterBar } from './TypeFilterBar'
import type { TrafficEntry, ProxyJumpTarget } from '@/types/proxy'
import type { AiConversation } from '@/types/ai'
import type { DetailPosition } from '@/features/bottom-bar'

interface ProxyViewProps {
  entries: TrafficEntry[]
  error: string
  showSidebar: boolean
  detailPosition: DetailPosition
  onAutoOpenDetail: () => void
  typeFilter: TypeFilter
  typeCounts: Map<TypeFilter, number>
  onTypeFilterChange: (f: TypeFilter) => void
  running: boolean
  status: string
  jumpTarget?: ProxyJumpTarget | null
  conversationOf?: (id: number) => AiConversation | undefined
}

export function ProxyView({
  entries,
  error,
  showSidebar,
  detailPosition,
  onAutoOpenDetail,
  typeFilter,
  typeCounts,
  onTypeFilterChange,
  running,
  status,
  jumpTarget,
  conversationOf,
}: ProxyViewProps) {
  return (
    <>
      <TypeFilterBar
        active={typeFilter}
        counts={typeCounts}
        onChange={onTypeFilterChange}
        running={running}
        status={status}
      />
      {error && (
        <Alert variant="destructive" className="shrink-0 border-0 rounded-none">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      <TrafficLog
        entries={entries}
        showSidebar={showSidebar}
        detailPosition={detailPosition}
        onAutoOpenDetail={onAutoOpenDetail}
        typeFilter={typeFilter}
        jumpTarget={jumpTarget}
        conversationOf={conversationOf}
      />
    </>
  )
}
