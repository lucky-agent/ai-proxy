import { useState, useEffect, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { TrafficEntry } from '@/types/proxy'
import type { AiConversation } from '@/types/ai'
import { isStreamingContentType } from '@/lib/sse'
import SidePanel, { type PanelTab, type TabDef } from './components/SidePanel'
import KeyValueTable from './components/KeyValueTable'
import BodyView from './components/BodyView'
import RawView from './components/RawView'
import StreamingViewer from './StreamingViewer'
import CompareView from './CompareView'
import { Empty, EmptyTitle } from '@/components/ui/empty'

interface Props {
  entry: TrafficEntry | undefined
  conversation?: AiConversation
  onTitleClick?: () => void
}

function parseResponseCookies(headers: Record<string, string>): Record<string, string> {
  const entry = Object.entries(headers).find(([k]) => k.toLowerCase() === 'set-cookie')
  if (!entry) return {}
  const result: Record<string, string> = {}
  entry[1].split('\n').forEach(line => {
    const trimmed = line.trim()
    if (!trimmed) return
    const semiIdx = trimmed.indexOf(';')
    const kvPart = semiIdx === -1 ? trimmed : trimmed.slice(0, semiIdx)
    const eqIdx = kvPart.indexOf('=')
    if (eqIdx > 0) {
      result[kvPart.slice(0, eqIdx).trim()] = kvPart.slice(eqIdx + 1).trim()
    }
  })
  return result
}

function formatResponseRaw(entry: TrafficEntry): string {
  if (!entry.responseHeaders) return ''
  const lines = [`HTTP/1.1 ${entry.status ?? '...'}`]
  for (const [key, value] of Object.entries(entry.responseHeaders)) {
    lines.push(`${key}: ${value}`)
  }
  if (entry.responseBody) {
    lines.push('', entry.responseBody)
  }
  return lines.join('\n')
}

export default function ResponsePanel({ entry, conversation, onTitleClick }: Props) {
  const { t } = useTranslation()
  const [tab, setTab] = useState<PanelTab>('header')

  const tabs = useMemo<TabDef[]>(() => {
    if (!entry) {
      return [
        { id: 'header', labelKey: 'detail.headers' },
        { id: 'body', labelKey: 'detail.body' },
        { id: 'raw', labelKey: 'detail.raw' },
        { id: 'console', labelKey: 'detail.console' },
      ]
    }
    const hasStream = (entry.responseChunks?.length ?? 0) > 1 || isStreamingContentType(entry.responseHeaders)
    const hasError = !!entry.error
    const base: TabDef[] = [
      { id: 'header', labelKey: 'detail.headers' },
      { id: 'cookies', labelKey: 'detail.cookies' },
      { id: 'body', labelKey: 'detail.body' },
    ]
    if (hasStream) {
      base.push({ id: 'stream', labelKey: 'detail.stream' })
    }
    base.push({ id: 'raw', labelKey: 'detail.raw' })
    if (hasError) {
      base.push({ id: 'console', labelKey: 'detail.console' })
    }
    if (conversation) {
      base.push({ id: 'compare', labelKey: 'detail.compare' })
    }
    return base
  }, [entry, conversation])

  // 切换条目时如果 stream tab 不再可用，切回 header
  useEffect(() => {
    if (tab === 'stream') {
      const hasStream = entry && ((entry.responseChunks?.length ?? 0) > 1 || isStreamingContentType(entry.responseHeaders))
      if (!hasStream) setTab('header')
    }
  }, [entry, tab])

  return (
    <SidePanel
      title={t('detail.response')}
      tab={tab}
      onTabChange={setTab}
      tabs={tabs}
      onTitleClick={onTitleClick}>
      <ResponsePanelContent tab={tab} entry={entry} conversation={conversation} t={t} onCloseStream={() => setTab('header')} />
    </SidePanel>
  )
}

function ResponsePanelContent({
  tab,
  entry,
  conversation,
  t,
  onCloseStream,
}: {
  tab: PanelTab
  entry: TrafficEntry | undefined
  conversation?: AiConversation
  t: (key: string) => string
  onCloseStream: () => void
}) {
  if (!entry) {
    if (tab === 'header' || tab === 'cookies') {
      return <KeyValueTable data={{}} emptyLabel="" />
    }
    return <Empty><EmptyTitle></EmptyTitle></Empty>
  }

  if (tab === 'header') {
    return entry.responseHeaders ? (
      <KeyValueTable data={entry.responseHeaders} emptyLabel={t('detail.noHeaders')} />
    ) : (
      <Empty><EmptyTitle>{t('detail.responsePending')}</EmptyTitle></Empty>
    )
  }

  if (tab === 'cookies') {
    const cookies = entry.responseHeaders ? parseResponseCookies(entry.responseHeaders) : {}
    return <KeyValueTable data={cookies} emptyLabel={t('detail.noCookies')} />
  }

  if (tab === 'body') {
    return entry.responseBody ? (
      <BodyView body={entry.responseBody} contentType={entry.responseContentType} />
    ) : (
      <Empty><EmptyTitle>{t('detail.noBody')}</EmptyTitle></Empty>
    )
  }

  if (tab === 'stream') {
    return <StreamingViewer entry={entry} onClose={onCloseStream} />
  }

  if (tab === 'console') {
    return entry.error ? (
      <RawView content={entry.error} />
    ) : (
      <Empty><EmptyTitle>{t('detail.noConsole')}</EmptyTitle></Empty>
    )
  }

  if (tab === 'compare') {
    if (!entry || !conversation) {
      return <Empty><EmptyTitle>{t('detail.compareWaiting')}</EmptyTitle></Empty>
    }
    return <CompareView entry={entry} conversation={conversation} />
  }

  // raw
  const content = formatResponseRaw(entry)
  return <RawView content={content} />
}
