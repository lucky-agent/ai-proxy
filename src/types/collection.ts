// src/types/collection.ts

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS'

export interface ApiCollection {
  id: string
  name: string
  children: ApiTreeNode[]
  createdAt: number
  updatedAt: number
}

export interface ApiFolderNode {
  id: string
  type: 'folder'
  name: string
  children: ApiTreeNode[]
}

export interface ApiRequestNode {
  id: string
  type: 'request'
  name: string
  method: HttpMethod
  url: string
  headers: { key: string; value: string }[]
  body: string
}

export type ApiTreeNode = ApiFolderNode | ApiRequestNode
