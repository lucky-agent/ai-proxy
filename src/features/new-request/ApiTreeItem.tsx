import { useState, useRef, useEffect, useCallback } from 'react'
import { ChevronRightIcon, FolderIcon, Trash2Icon, CopyIcon, PencilIcon, FileIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ApiTreeNode, ApiFolderNode, ApiRequestNode } from '@/types/collection'
import { useLocale } from '@/hooks/useLocale'

const METHOD_COLORS: Record<string, string> = {
  GET: 'text-badge-get',
  POST: 'text-badge-post',
  PUT: 'text-badge-put',
  DELETE: 'text-badge-delete',
  PATCH: 'text-badge-patch',
  HEAD: 'text-badge-head',
  OPTIONS: 'text-badge-options',
}

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
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null)
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

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setContextMenu({ x: e.clientX, y: e.clientY })
  }, [])

  const closeContextMenu = useCallback(() => {
    setContextMenu(null)
  }, [])

  // 点击事件
  const handleClick = useCallback(() => {
    if (isFolder) {
      onToggleExpand(node.id)
    } else {
      onSelectRequest(node as ApiRequestNode)
    }
  }, [isFolder, node, onToggleExpand, onSelectRequest])

  return (
    <>
      <div
        className={cn(
          'group flex items-center gap-1 px-2 py-1 cursor-pointer rounded-sm text-xs transition-colors',
          isSelected ? 'bg-primary/10 text-foreground' : 'text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50',
        )}
        style={{ paddingLeft: `${depth * 12 + 8}px` }}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
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
            <span className={cn('shrink-0 text-[10px] font-bold', METHOD_COLORS[(node as ApiRequestNode).method] || 'text-muted-foreground')}>
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
          <input
            ref={renameInputRef}
            value={renameValue}
            onChange={e => setRenameValue(e.target.value)}
            onBlur={handleRenameSubmit}
            onKeyDown={e => {
              if (e.key === 'Enter') handleRenameSubmit()
              if (e.key === 'Escape') { setRenaming(false); setRenameValue(node.name) }
            }}
            className="flex-1 min-w-0 rounded border border-input bg-background px-1 py-0 text-xs font-mono outline-none focus:ring-1 focus:ring-primary"
          />
        )}
      </div>

      {/* 文件夹展开时渲染子节点 */}
      {isFolder && isExpanded && (node as ApiFolderNode).children.map(child => (
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

      {/* 右键菜单 */}
      {contextMenu && (
        <div
          className="fixed z-50 rounded-md border border-border bg-surface-base shadow-md py-1 text-xs"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={closeContextMenu}
        >
          {isFolder ? (
            <>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated text-foreground" onClick={() => { onAddRequest(node.id); closeContextMenu() }}>
                <FileIcon className="size-3" /> {t('collection.newRequest')}
              </button>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated text-foreground" onClick={() => { onAddFolder(node.id); closeContextMenu() }}>
                <FolderIcon className="size-3" /> {t('collection.newFolder')}
              </button>
              <div className="mx-1 my-1 border-t border-border" />
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated text-foreground" onClick={() => { setRenaming(true); closeContextMenu() }}>
                <PencilIcon className="size-3" /> {t('collection.rename')}
              </button>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated text-destructive" onClick={() => { onRemoveNode(node.id); closeContextMenu() }}>
                <Trash2Icon className="size-3" /> {t('collection.delete')}
              </button>
            </>
          ) : (
            <>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated text-foreground" onClick={() => { setRenaming(true); closeContextMenu() }}>
                <PencilIcon className="size-3" /> {t('collection.rename')}
              </button>
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated text-foreground" onClick={() => { onDuplicateRequest(node.id); closeContextMenu() }}>
                <CopyIcon className="size-3" /> {t('collection.duplicate')}
              </button>
              <div className="mx-1 my-1 border-t border-border" />
              <button className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-surface-elevated text-destructive" onClick={() => { onRemoveNode(node.id); closeContextMenu() }}>
                <Trash2Icon className="size-3" /> {t('collection.delete')}
              </button>
            </>
          )}
        </div>
      )}
    </>
  )
}
