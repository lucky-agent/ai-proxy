// src/types/collection.ts

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS'

export type BodyType = 'json' | 'xml' | 'text' | 'urlencoded' | 'multipart'

export interface KeyValuePair {
  key: string
  value: string
}

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
  params: KeyValuePair[]
  headers: KeyValuePair[]
  cookies: KeyValuePair[]
  bodyType: BodyType
  body: string
}

export type ApiTreeNode = ApiFolderNode | ApiRequestNode

export interface RequestTab {
  id: string
  name: string
  linkedNodeId: string | null
  method: HttpMethod
  url: string
  params: KeyValuePair[]
  headers: KeyValuePair[]
  cookies: KeyValuePair[]
  bodyType: BodyType
  body: string
  responseEntryId: string | null
  sending: boolean
  error: string
}
