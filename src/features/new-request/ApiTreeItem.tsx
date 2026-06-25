import { useState, useRef, useEffect, useCallback } from 'react'
import { ChevronRightIcon, FolderIcon, Trash2Icon, CopyIcon, PencilIcon, FileIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import type { ApiTreeNode, ApiFolderNode, ApiRequestNode } from '@/types/collection'
import { METHOD_COLORS } from '@/lib/http-constants'
import { useLocale } from '@/hooks/useLocale'

interface ApiTreeItemProps {
  node: ApiTreeNode
  depth: number
  selectedId: string | null
  onSelectRequest: (node: ApiRequestNode) => void
  onRemoveNode: (nodeId: string) => void
  onRenameNode: (nodeId: string, newName: string) => void
  onDuplicateRequest: (nodeId: string) => void
  onAddFolder: (parentId: string) => void
  onAddRequest: (parentId: string) => void
  expandedIds: Set<string>
  onToggleExpand: (nodeId: string) => void
}

export function ApiTreeItem({
  node,
  depth,
  selectedId,
  onSelectRequest,
  onRemoveNode,
  onRenameNode,
  onDuplicateRequest,
  onAddFolder,
  onAddRequest,
  expandedIds,
  onToggleExpand,
}: ApiTreeItemProps) {
  const { t } = useLocale()
  const [renaming, setRenaming] = useState(false)
  const [renameValue, setRenameValue] = useState(node.name)
  const [contextMenuOpen, setContextMenuOpen] = useState(false)
  const renameInputRef = useRef<HTMLInputElement>(null)

  const isFolder = node.type === 'folder'
  const isSelected = !isFolder && selectedId === node.id
  const isExpanded = isFolder && expandedIds.has(node.id)

  // 重命名时自动 focus
  useEffect(() => {
    if (renaming && renameInputRef.current) {
      renameInputRef.current.focus()
      renameInputRef.current.select()
    }
  }, [renaming])

  const handleRenameSubmit = useCallback(() => {
    const trimmed = renameValue.trim()
    if (trimmed && trimmed !== node.name) {
      onRenameNode(node.id, trimmed)
    }
    setRenaming(false)
  }, [renameValue, node.id, node.name, onRenameNode])

  // 点击事件
  const handleClick = useCallback(() => {
    if (isFolder) {
      onToggleExpand(node.id)
    } else {
      onSelectRequest(node as ApiRequestNode)
    }
  }, [isFolder, node, onToggleExpand, onSelectRequest])

  return (
    <ContextMenu onOpenChange={setContextMenuOpen}>
      <ContextMenuTrigger>
        <div
          className={cn(
            'group flex items-center gap-1 px-2 py-1 cursor-pointer rounded-sm text-xs transition-colors',
            isSelected ? 'bg-primary/10 text-foreground' : 'text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50',
            contextMenuOpen && 'bg-surface-elevated/50 text-foreground'
          )}
          style={{ paddingLeft: `${depth * 12 + 8}px` }}
          onClick={handleClick}
          onDoubleClick={() => setRenaming(true)}
        >
          {/* 文件夹：展开/折叠箭头 */}
          {isFolder && (
            <ChevronRightIcon
              className={cn('size-3 shrink-0 transition-transform', isExpanded && 'rotate-90')}
            />
          )}
          {/* 请求：无箭头，留占位 */}
          {!isFolder && <span className="w-3 shrink-0" />}

          {/* 文件夹图标 */}
          {isFolder && <FolderIcon className="size-3.5 shrink-0 text-muted-foreground" />}

          {/* 请求：Method badge + 名称 */}
          {!isFolder && !renaming && (
            <>
              <span className={cn('shrink-0 text-[10px] font-bold', `text-${METHOD_COLORS[(node as ApiRequestNode).method]}`)}>
                {(node as ApiRequestNode).method}
              </span>
              <span className="truncate">{node.name}</span>
            </>
          )}

          {/* 文件夹：名称 */}
          {isFolder && !renaming && (
            <span className="truncate">{node.name}</span>
          )}

          {/* 重命名输入框 */}
          {renaming && (
            <Input
              ref={renameInputRef}
              value={renameValue}
              onChange={e => setRenameValue(e.target.value)}
              onBlur={handleRenameSubmit}
              onKeyDown={e => {
                if (e.key === 'Enter') handleRenameSubmit()
                if (e.key === 'Escape') { setRenaming(false); setRenameValue(node.name) }
              }}
              className="flex-1 min-w-0 h-auto py-0 text-xs font-mono"
            />
          )}
        </div>
      </ContextMenuTrigger>

      <ContextMenuContent className="text-xs min-w-32">
        {isFolder ? (
          <>
            <ContextMenuItem onClick={() => onAddRequest(node.id)}>
              <FileIcon className="size-3" />
              <span>{t('collection.newRequest')}</span>
            </ContextMenuItem>
            <ContextMenuItem onClick={() => onAddFolder(node.id)}>
              <FolderIcon className="size-3" />
              <span>{t('collection.newFolder')}</span>
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem onClick={() => setRenaming(true)}>
              <PencilIcon className="size-3" />
              <span>{t('collection.rename')}</span>
            </ContextMenuItem>
            <ContextMenuItem variant="destructive" onClick={() => onRemoveNode(node.id)}>
              <Trash2Icon className="size-3" />
              <span>{t('collection.delete')}</span>
            </ContextMenuItem>
          </>
        ) : (
          <>
            <ContextMenuItem onClick={() => setRenaming(true)}>
              <PencilIcon className="size-3" />
              <span>{t('collection.rename')}</span>
            </ContextMenuItem>
            <ContextMenuItem onClick={() => onDuplicateRequest(node.id)}>
              <CopyIcon className="size-3" />
              <span>{t('collection.duplicate')}</span>
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem variant="destructive" onClick={() => onRemoveNode(node.id)}>
              <Trash2Icon className="size-3" />
              <span>{t('collection.delete')}</span>
            </ContextMenuItem>
          </>
        )}
      </ContextMenuContent>

      {/* 递归渲染子节点 */}
      {isFolder && isExpanded && (
        <div>
          {(node as ApiFolderNode).children.map(child => (
            <ApiTreeItem
              key={child.id}
              node={child}
              depth={depth + 1}
              selectedId={selectedId}
              onSelectRequest={onSelectRequest}
              onRemoveNode={onRemoveNode}
              onRenameNode={onRenameNode}
              onDuplicateRequest={onDuplicateRequest}
              onAddFolder={onAddFolder}
              onAddRequest={onAddRequest}
              expandedIds={expandedIds}
              onToggleExpand={onToggleExpand}
            />
          ))}
        </div>
      )}
    </ContextMenu>
  )
}
