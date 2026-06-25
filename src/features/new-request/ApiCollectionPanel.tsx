// src/features/new-request/ApiCollectionPanel.tsx
import { useState, useCallback } from 'react'
import { RefreshCwIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import type { ApiCollection, ApiRequestNode } from '@/types/collection'
import { ApiTreeView } from './ApiTreeView'
import { Separator } from '@/components/ui/separator'

interface ApiCollectionPanelProps {
  collections: ApiCollection[]
  selectedId: string | null
  onSelectRequest: (node: ApiRequestNode) => void
  addFolder: (parentId: string) => void
  addRequest: (parentId: string) => void
  removeNode: (nodeId: string) => void
  renameNode: (nodeId: string, newName: string) => void
  duplicateRequest: (nodeId: string) => void
  renameCollection: (collectionId: string, newName: string) => void
  onRefresh: () => void
}

export function ApiCollectionPanel({
  collections,
  selectedId,
  onSelectRequest,
  addFolder,
  addRequest,
  removeNode,
  renameNode,
  duplicateRequest,
  renameCollection,
  onRefresh,
}: ApiCollectionPanelProps) {
  const { t } = useLocale()
  const [spinning, setSpinning] = useState(false)

  const handleRefresh = useCallback(() => {
    setSpinning(true)
    onRefresh()
    setTimeout(() => setSpinning(false), 600)
  }, [onRefresh])

  return (
    <div className="flex h-full flex-col bg-surface-base/30">
      {/* 标题栏 */}
      <div className="flex items-center px-3 py-2">
        <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground flex-1">
          {t('collection.title')}
        </span>
        <button
          onClick={handleRefresh}
          className="text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
        >
          <RefreshCwIcon className={cn('size-3.5', spinning && 'animate-spin')} />
        </button>
      </div>

      <Separator />

      {/* 树形菜单 */}
      <ApiTreeView
        collections={collections}
        selectedId={selectedId}
        onSelectRequest={onSelectRequest}
        onRemoveNode={removeNode}
        onRenameNode={renameNode}
        onDuplicateRequest={duplicateRequest}
        onAddFolder={addFolder}
        onAddRequest={addRequest}
        onRenameCollection={renameCollection}
      />
    </div>
  )
}
