// src/hooks/useCollections.ts
import { useState, useEffect, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { ApiCollection, ApiTreeNode, ApiFolderNode, ApiRequestNode, HttpMethod, BodyType, KeyValuePair } from '@/types/collection'

function generateId(): string {
  return crypto.randomUUID()
}

/** Normalize a request node with defaults for backward compatibility */
function normalizeRequest(node: ApiRequestNode): ApiRequestNode {
  return {
    ...node,
    params: node.params ?? [],
    cookies: node.cookies ?? [],
    bodyType: node.bodyType ?? 'json',
    headers: node.headers ?? [],
    body: node.body ?? '',
  }
}

/** Recursively normalize all nodes in a tree */
function normalizeTree(nodes: ApiTreeNode[]): ApiTreeNode[] {
  return nodes.map(node => {
    if (node.type === 'request') return normalizeRequest(node as ApiRequestNode)
    if (node.type === 'folder') return { ...node, children: normalizeTree(node.children) }
    return node
  })
}

function createDefaultCollection(): ApiCollection {
  return {
    id: generateId(),
    name: '默认集合',
    children: [],
    createdAt: Date.now(),
    updatedAt: Date.now(),
  }
}

/** 递归查找节点并替换（immutable） */
function updateNodeInTree(
  nodes: ApiTreeNode[],
  nodeId: string,
  updater: (node: ApiTreeNode) => ApiTreeNode,
): ApiTreeNode[] {
  return nodes.map(node => {
    if (node.id === nodeId) {
      return updater(node)
    }
    if (node.type === 'folder') {
      return {
        ...node,
        children: updateNodeInTree(node.children, nodeId, updater),
      }
    }
    return node
  })
}

/** 递归查找节点并删除（immutable） */
function removeNodeFromTree(nodes: ApiTreeNode[], nodeId: string): ApiTreeNode[] {
  return nodes
    .filter(node => node.id !== nodeId)
    .map(node => {
      if (node.type === 'folder') {
        return { ...node, children: removeNodeFromTree(node.children, nodeId) }
      }
      return node
    })
}

/** 递归在文件夹下插入节点（immutable） */
function insertNodeInTree(
  nodes: ApiTreeNode[],
  parentId: string,
  newNode: ApiTreeNode,
): ApiTreeNode[] {
  return nodes.map(node => {
    if (node.id === parentId && node.type === 'folder') {
      return { ...node, children: [...node.children, newNode] }
    }
    if (node.type === 'folder') {
      return { ...node, children: insertNodeInTree(node.children, parentId, newNode) }
    }
    return node
  })
}

export function useCollections() {
  const [collections, setCollections] = useState<ApiCollection[]>([])
  const [loading, setLoading] = useState(true)
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // 加载
  useEffect(() => {
    invoke<(ApiCollection & { children: ApiTreeNode[] })[]>('get_collections')
      .then(data => {
        if (data.length === 0) {
          const default_ = createDefaultCollection()
          setCollections([default_])
          invoke('save_collections', { collections: [default_] }).catch(console.error)
        } else {
          // Normalize all nodes for backward compatibility
          const normalized: ApiCollection[] = data.map(c => ({
            ...c,
            children: normalizeTree(c.children),
            updatedAt: c.updatedAt ?? c.createdAt,
          }))
          setCollections(normalized)
        }
      })
      .catch(console.error)
      .finally(() => setLoading(false))
  }, [])

  // 延迟保存（避免每次操作都 invoke）
  const debouncedSave = useCallback((data: ApiCollection[]) => {
    if (saveTimer.current) clearTimeout(saveTimer.current)
    saveTimer.current = setTimeout(() => {
      invoke('save_collections', { collections: data }).catch(console.error)
    }, 300)
  }, [])

  const updateCollections = useCallback((updater: (prev: ApiCollection[]) => ApiCollection[]) => {
    setCollections(prev => {
      const next = updater(prev)
      debouncedSave(next)
      return next
    })
  }, [debouncedSave])

  // --- 操作方法 ---

  /** 在根 Collection 或某文件夹下添加文件夹 */
  const addFolder = useCallback((parentId: string) => {
    const newFolder: ApiFolderNode = {
      id: generateId(),
      type: 'folder',
      name: '新建文件夹',
      children: [],
    }
    updateCollections(prev =>
      prev.map(col => {
        if (col.id === parentId) {
          return { ...col, children: [...col.children, newFolder], updatedAt: Date.now() }
        }
        return {
          ...col,
          children: insertNodeInTree(col.children, parentId, newFolder),
          updatedAt: Date.now(),
        }
      }),
    )
  }, [updateCollections])

  /** 在根 Collection 或某文件夹下添加请求 */
  const addRequest = useCallback((parentId: string) => {
    const newRequest: ApiRequestNode = {
      id: generateId(),
      type: 'request',
      name: '新建请求',
      method: 'GET',
      url: '',
      params: [],
      headers: [],
      cookies: [],
      bodyType: 'json',
      body: '',
    }
    updateCollections(prev =>
      prev.map(col => {
        if (col.id === parentId) {
          return { ...col, children: [...col.children, newRequest], updatedAt: Date.now() }
        }
        return {
          ...col,
          children: insertNodeInTree(col.children, parentId, newRequest),
          updatedAt: Date.now(),
        }
      }),
    )
  }, [updateCollections])

  /** 删除节点 */
  const removeNode = useCallback((nodeId: string) => {
    updateCollections(prev =>
      prev.map(col => ({
        ...col,
        children: removeNodeFromTree(col.children, nodeId),
        updatedAt: Date.now(),
      })),
    )
  }, [updateCollections])

  /** 重命名节点 */
  const renameNode = useCallback((nodeId: string, newName: string) => {
    updateCollections(prev =>
      prev.map(col => ({
        ...col,
        children: updateNodeInTree(col.children, nodeId, node =>
          ({ ...node, name: newName }),
        ),
        updatedAt: Date.now(),
      })),
    )
  }, [updateCollections])

  /** 更新请求节点配置 */
  const updateRequest = useCallback(
    (nodeId: string, data: {
      method?: HttpMethod
      url?: string
      params?: KeyValuePair[]
      headers?: KeyValuePair[]
      cookies?: KeyValuePair[]
      bodyType?: BodyType
      body?: string
    }) => {
      updateCollections(prev =>
        prev.map(col => ({
          ...col,
          children: updateNodeInTree(col.children, nodeId, node => {
            if (node.type !== 'request') return node
            return { ...node, ...data }
          }),
          updatedAt: Date.now(),
        })),
      )
    },
    [updateCollections],
  )

  /** 复制请求节点 */
  const duplicateRequest = useCallback((nodeId: string) => {
    updateCollections(prev => {
      const findNode = (nodes: ApiTreeNode[]): ApiRequestNode | null => {
        for (const n of nodes) {
          if (n.id === nodeId && n.type === 'request') return n
          if (n.type === 'folder') {
            const found = findNode(n.children)
            if (found) return found
          }
        }
        return null
      }

      const insertCopy = (nodes: ApiTreeNode[], copy: ApiRequestNode): ApiTreeNode[] => {
        if (nodes.some(n => n.id === nodeId)) {
          const idx = nodes.findIndex(n => n.id === nodeId)
          return [...nodes.slice(0, idx + 1), copy, ...nodes.slice(idx + 1)]
        }
        return nodes.map(n => {
          if (n.type === 'folder') {
            return { ...n, children: insertCopy(n.children, copy) }
          }
          return n
        })
      }

      const orig = prev.flatMap(c => findNode(c.children) ? [findNode(c.children)!] : [])
      if (orig.length === 0) return prev
      const o = orig[0]

      const copy: ApiRequestNode = {
        ...o,
        id: generateId(),
        name: o.name + ' (副本)',
      }

      return prev.map(col => ({
        ...col,
        children: insertCopy(col.children, copy),
        updatedAt: Date.now(),
      }))
    })
  }, [updateCollections])

  /** 重命名 Collection */
  const renameCollection = useCallback((collectionId: string, newName: string) => {
    updateCollections(prev =>
      prev.map(col =>
        col.id === collectionId ? { ...col, name: newName, updatedAt: Date.now() } : col,
      ),
    )
  }, [updateCollections])

  return {
    collections,
    loading,
    addFolder,
    addRequest,
    removeNode,
    renameNode,
    updateRequest,
    duplicateRequest,
    renameCollection,
  }
}
