// src/types/collection.ts

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS'

export type BodyType = 'json' | 'xml' | 'text' | 'urlencoded' | 'multipart'

export interface KeyValuePair {
  key: string
  value: string
}

export interface ApiCollection {
  id: number
  name: string
  children: ApiTreeNode[]
  createdAt: number
  updatedAt: number
}

export interface ApiFolderNode {
  id: number
  type: 'folder'
  name: string
  children: ApiTreeNode[]
}

export interface ApiRequestNode {
  id: number
  type: 'request'
  name: string
  method: HttpMethod
  url: string
  params: KeyValuePair[]
  headers: KeyValuePair[]
  cookies: KeyValuePair[]
  bodyType: BodyType
  body: string
  authType?: string
  authData?: string
  requestId?: number
}

export type ApiTreeNode = ApiFolderNode | ApiRequestNode

/** Subset of RequestTab fields that constitute saved request data */
export interface RequestTabSavedData {
  method: HttpMethod
  url: string
  params: KeyValuePair[]
  headers: KeyValuePair[]
  cookies: KeyValuePair[]
  bodyType: BodyType
  body: string
  authType: string
  authData: string
}

export interface RequestTab {
  id: string
  name: string
  linkedNodeId: number | null
  dirty: boolean
  /** Last saved data snapshot — dirty=false when current fields match this; null for unlinked tabs */
  savedData: RequestTabSavedData | null
  method: HttpMethod
  url: string
  params: KeyValuePair[]
  headers: KeyValuePair[]
  cookies: KeyValuePair[]
  bodyType: BodyType
  body: string
  authType: string
  authData: string
  responseEntryId: number | null
  sending: boolean
  error: string
}
