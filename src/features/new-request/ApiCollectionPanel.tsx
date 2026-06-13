// src/features/new-request/ApiCollectionPanel.tsx
import { useCallback } from 'react'
import { FolderPlusIcon, PlusIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import type { ApiCollection, ApiRequestNode } from '@/types/collection'
import { ApiTreeView } from './ApiTreeView'
import { Separator } from '@/components/ui/separator'
import { ScrollArea } from '@/components/ui/scroll-area'

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
}: ApiCollectionPanelProps) {
  const { t } = useLocale()

  // 默认添加到第一个 Collection 的根层
  const defaultCollectionId = collections[0]?.id ?? ''
  const handleAddFolder = useCallback(() => addFolder(defaultCollectionId), [addFolder, defaultCollectionId])
  const handleAddRequest = useCallback(() => addRequest(defaultCollectionId), [addRequest, defaultCollectionId])

  return (
    <div className="flex h-full flex-col bg-surface-base/30">
      {/* 标题栏 */}
      <div className="flex items-center px-3 py-2">
        <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
          {t('collection.title')}
        </span>
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

      <Separator />

      {/* 操作按钮 */}
      <div className="flex items-center gap-1 px-3 py-2">
        <button
          onClick={handleAddFolder}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <FolderPlusIcon className="size-3.5" />
          {t('collection.newFolder')}
        </button>
        <button
          onClick={handleAddRequest}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <PlusIcon className="size-3.5" />
          {t('collection.newRequest')}
        </button>
      </div>
    </div>
  )
}
