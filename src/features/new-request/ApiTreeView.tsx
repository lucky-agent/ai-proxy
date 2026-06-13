// src/features/new-request/ApiTreeView.tsx
import { useState, useCallback } from 'react'
import type { ApiCollection, ApiRequestNode } from '@/types/collection'
import { useLocale } from '@/hooks/useLocale'
import { ApiTreeItem } from './ApiTreeItem'
import { ScrollArea } from '@/components/ui/scroll-area'

interface ApiTreeViewProps {
  collections: ApiCollection[]
  selectedId: string | null
  onSelectRequest: (node: ApiRequestNode) => void
  onRemoveNode: (nodeId: string) => void
  onRenameNode: (nodeId: string, newName: string) => void
  onDuplicateRequest: (nodeId: string) => void
  onAddFolder: (parentId: string) => void
  onAddRequest: (parentId: string) => void
  onRenameCollection: (collectionId: string, newName: string) => void
}

export function ApiTreeView({
  collections,
  selectedId,
  onSelectRequest,
  onRemoveNode,
  onRenameNode,
  onDuplicateRequest,
  onAddFolder,
  onAddRequest,
  onRenameCollection,
}: ApiTreeViewProps) {
  const { t } = useLocale()
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => {
    // 默认展开所有 Collection 根节点
    return new Set(collections.map(c => c.id))
  })

  const handleToggleExpand = useCallback((nodeId: string) => {
    setExpandedIds(prev => {
      const next = new Set(prev)
      if (next.has(nodeId)) next.delete(nodeId)
      else next.add(nodeId)
      return next
    })
  }, [])

  return (
    <ScrollArea className="flex-1 min-h-0 py-1">
      {collections.map(col => (
        <div key={col.id}>
          {/* Collection 根节点作为 folder 类型渲染 */}
          <ApiTreeItem
            node={{
              id: col.id,
              type: 'folder',
              name: col.name,
              children: col.children,
            }}
            depth={0}
            selectedId={selectedId}
            onSelectRequest={onSelectRequest}
            onRemoveNode={onRemoveNode}
            onRenameNode={(nodeId, newName) => {
              // 根节点重命名走 renameCollection
              if (nodeId === col.id) {
                onRenameCollection(col.id, newName)
              } else {
                onRenameNode(nodeId, newName)
              }
            }}
            onDuplicateRequest={onDuplicateRequest}
            onAddFolder={onAddFolder}
            onAddRequest={onAddRequest}
            expandedIds={expandedIds}
            onToggleExpand={handleToggleExpand}
          />
        </div>
      ))}

      {/* 空状态 */}
      {collections.length === 0 && (
        <div className="px-4 py-6 text-xs text-muted-foreground text-center">
          {t('collection.emptyTree')}
        </div>
      )}
    </ScrollArea>
  )
}
