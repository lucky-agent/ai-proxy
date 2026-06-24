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

/** Find the root collection id that contains a given node */
function findCollectionIdForNode(collections: ApiCollection[], nodeId: string): string | null {
  for (const col of collections) {
    if (col.id === nodeId) return col.id
    if (nodeInTree(col.children, nodeId)) return col.id
  }
  return null
}

function nodeInTree(nodes: ApiTreeNode[], targetId: string): boolean {
  for (const n of nodes) {
    if (n.id === targetId) return true
    if (n.type === 'folder' && nodeInTree(n.children, targetId)) return true
  }
  return false
}

/** Safely cast a node to ApiRequestNode */
function asRequest(node: ApiTreeNode): ApiRequestNode | null {
  return node.type === 'request' ? (node as ApiRequestNode) : null
}

export function useCollections() {
  const [collections, setCollections] = useState<ApiCollection[]>([])
  const [loading, setLoading] = useState(true)

  // 7.6: Keep a debounced save timer for updateRequest (keystroke-level debounce)
  const saveRequestTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // 加载
  useEffect(() => {
    invoke<(ApiCollection & { children: ApiTreeNode[] })[]>('get_collections')
      .then(data => {
        if (data.length === 0) {
          setCollections([])
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

  // 7.1: Clean up on unmount
  useEffect(() => {
    return () => {
      if (saveRequestTimer.current) {
        clearTimeout(saveRequestTimer.current)
      }
    }
  }, [])

  // --- 操作方法 ---

  /** 7.2: Add a folder under a collection or another folder */
  const addFolder = useCallback((parentId: string) => {
    const newFolder: ApiFolderNode = {
      id: generateId(),
      type: 'folder',
      name: '新建文件夹',
      children: [],
    }
    // Optimistic update
    setCollections(prev =>
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
    // Fire-and-forget backend call
    invoke('create_folder', { parentId, name: '新建文件夹' }).catch(console.error)
  }, [])

  /** 7.3: Add a request under a parent node (collection or folder) */
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
    // Optimistic update
    setCollections(prev => {
      // Find the collection id for this parent
      const collectionId = findCollectionIdForNode(prev, parentId)

      // Fire-and-forget backend call
      if (collectionId) {
        invoke('create_request', { parentId, collectionId, name: '新建请求' }).catch(console.error)
      }

      return prev.map(col => {
        if (col.id === parentId) {
          return { ...col, children: [...col.children, newRequest], updatedAt: Date.now() }
        }
        return {
          ...col,
          children: insertNodeInTree(col.children, parentId, newRequest),
          updatedAt: Date.now(),
        }
      })
    })
  }, [])

  /** 7.4: Delete a node */
  const removeNode = useCallback((nodeId: string) => {
    // Optimistic update
    setCollections(prev =>
      prev.map(col => ({
        ...col,
        children: removeNodeFromTree(col.children, nodeId),
        updatedAt: Date.now(),
      })),
    )
    // Fire-and-forget backend call
    invoke('delete_node', { nodeId }).catch(console.error)
  }, [])

  /** 7.5: Rename a node */
  const renameNode = useCallback((nodeId: string, newName: string) => {
    // Optimistic update
    setCollections(prev =>
      prev.map(col => ({
        ...col,
        children: updateNodeInTree(col.children, nodeId, node =>
          ({ ...node, name: newName }),
        ),
        updatedAt: Date.now(),
      })),
    )
    // Fire-and-forget backend call
    invoke('rename_node', { nodeId, newName }).catch(console.error)
  }, [])

  /** 7.6: Update request node data — debounced for keystroke-level writes */
  const updateRequest = useCallback(
    (nodeId: string, data: {
      method?: HttpMethod
      url?: string
      params?: KeyValuePair[]
      headers?: KeyValuePair[]
      cookies?: KeyValuePair[]
      bodyType?: BodyType
      body?: string
      authType?: string
      authData?: string
    }) => {
      // Always update React state immediately (optimistic)
      setCollections(prev =>
        prev.map(col => ({
          ...col,
          children: updateNodeInTree(col.children, nodeId, node => {
            if (node.type !== 'request') return node
            return { ...node, ...data }
          }),
          updatedAt: Date.now(),
        })),
      )

      // Debounced backend write: we need requestId from the node
      if (saveRequestTimer.current) clearTimeout(saveRequestTimer.current)
      saveRequestTimer.current = setTimeout(() => {
        // Read current collections to find the requestId
        setCollections(prev => {
          // Find the node and its requestId across all collections
          for (const col of prev) {
            const req = findRequestInNodes(col.children, nodeId)
            if (req) {
              invoke('save_request', {
                id: req.requestId ?? '',
                method: data.method ?? req.method,
                url: data.url ?? req.url,
                headers: data.headers ?? req.headers,
                params: data.params ?? req.params,
                body: data.body ?? req.body,
                bodyType: data.bodyType ?? req.bodyType,
                cookies: data.cookies ?? req.cookies,
                authType: data.authType ?? req.authType ?? '',
                authData: data.authData ?? req.authData ?? '',
              }).catch(console.error)
              break
            }
          }
          return prev
        })
      }, 300)
    },
    [],
  )

  /** 7.7: Duplicate a request node */
  const duplicateRequest = useCallback((nodeId: string) => {
    // Optimistic backend call — backend creates the duplicate and returns new nodeId
    invoke<string>('duplicate_request', { nodeId })
      .then(newNodeId => {
        // After successful backend op, reload collections to get accurate state
        invoke<(ApiCollection & { children: ApiTreeNode[] })[]>('get_collections')
          .then(data => {
            const normalized: ApiCollection[] = data.map(c => ({
              ...c,
              children: normalizeTree(c.children),
              updatedAt: c.updatedAt ?? c.createdAt,
            }))
            setCollections(normalized)
          })
          .catch(console.error)
      })
      .catch(console.error)
  }, [])

  /** 7.8: Rename a collection (uses rename_node command under the hood) */
  const renameCollection = useCallback((collectionId: string, newName: string) => {
    // Optimistic update
    setCollections(prev =>
      prev.map(col =>
        col.id === collectionId ? { ...col, name: newName, updatedAt: Date.now() } : col,
      ),
    )
    // Fire-and-forget backend call — collection is also a node in collection_nodes
    invoke('rename_node', { nodeId: collectionId, newName }).catch(console.error)
  }, [])

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

/** Recursively find a request node by nodeId in a tree */
function findRequestInNodes(nodes: ApiTreeNode[], targetId: string): ApiRequestNode | null {
  for (const n of nodes) {
    if (n.type === 'request' && n.id === targetId) {
      return asRequest(n)
    }
    if (n.type === 'folder') {
      const found = findRequestInNodes(n.children, targetId)
      if (found) return found
    }
  }
  return null
}
