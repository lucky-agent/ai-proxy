import { SparklesIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import { AiSidebar } from './AiSidebar'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { usePanelRef } from 'react-resizable-panels'
import { useEffect } from 'react'
import type { DetailPosition } from '@/features/bottom-bar'

interface AiViewProps {
  showSidebar: boolean
  detailPosition: DetailPosition
}

export function AiView({ showSidebar }: AiViewProps) {
  const { t } = useLocale()

  const aiSidebarPanelRef = usePanelRef()
  useEffect(() => {
    const panel = aiSidebarPanelRef.current
    if (!panel) return
    if (showSidebar) {
      panel.resize("22%")
    } else {
      panel.collapse()
    }
  }, [showSidebar])

  return (
    <ResizablePanelGroup orientation="horizontal" id="ai-view" className="h-full bg-surface-deep">
      {/* Left: AI sidebar */}
      <ResizablePanel id="ai-sidebar" defaultSize="22%" minSize="15%" maxSize="40%" collapsible collapsedSize={0} panelRef={aiSidebarPanelRef}>
        <div className="h-full overflow-hidden">
          <AiSidebar />
        </div>
      </ResizablePanel>

      <ResizableHandle withHandle />

      {/* Right: main AI content */}
      <ResizablePanel id="ai-main" defaultSize="78%" minSize="60%">
        <div className="flex h-full flex-col items-center justify-center gap-3 bg-surface-deep text-muted-foreground">
          <SparklesIcon className="size-12 text-muted-foreground/30" />
          <p className="text-sm font-medium">{t('view.aiComingSoon')}</p>
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}
