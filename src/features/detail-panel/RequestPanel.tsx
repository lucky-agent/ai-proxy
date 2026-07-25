import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import type { TrafficEntry } from '@/types/proxy'
import SidePanel, { type PanelTab, type TabDef } from './components/SidePanel'
import KeyValueTable from './components/KeyValueTable'
import BodyView from './components/BodyView'
import RawView from './components/RawView'
import FormDataView from './FormDataView'
import { Empty, EmptyTitle } from '@/components/ui/empty'

interface Props {
  entry: TrafficEntry | undefined
  onTitleClick?: () => void
}

/** requestSide tabs — depends on whether the entry has query params */
function buildTabs(hasQuery: boolean): TabDef[] {
  const tabs: TabDef[] = [
    { id: 'header', labelKey: 'detail.headers' },
  ]
  if (hasQuery) tabs.push({ id: 'query', labelKey: 'detail.query' })
  tabs.push({ id: 'cookies', labelKey: 'detail.cookies' })
  tabs.push({ id: 'form', labelKey: 'detail.formData' })
  tabs.push(
    { id: 'body', labelKey: 'detail.body' },
    { id: 'raw', labelKey: 'detail.raw' },
  )
  return tabs
}

function parseCookies(headers: Record<string, string>): Record<string, string> {
  const entry = Object.entries(headers).find(([k]) => k.toLowerCase() === 'cookie')
  if (!entry) return {}
  const result: Record<string, string> = {}
  entry[1].split(';').forEach(pair => {
    const trimmed = pair.trim()
    const eqIdx = trimmed.indexOf('=')
    if (eqIdx > 0) {
      result[trimmed.slice(0, eqIdx).trim()] = trimmed.slice(eqIdx + 1).trim()
    }
  })
  return result
}

function formatRequestRaw(entry: TrafficEntry): string {
  const lines = [`${entry.method} ${entry.uri} HTTP/1.1`]
  for (const [key, value] of Object.entries(entry.requestHeaders)) {
    lines.push(`${key}: ${value}`)
  }
  if (entry.requestBody) {
    lines.push('', entry.requestBody)
  }
  return lines.join('\n')
}

export default function RequestPanel({ entry, onTitleClick }: Props) {
  const { t } = useTranslation()
  const hasQuery = entry ? !!entry.requestQuery && Object.keys(entry.requestQuery).length > 0 : false
  const tabs = buildTabs(hasQuery)
  const [tab, setTab] = useState<PanelTab>('body')

  // 切换条目时回退到 body
  useEffect(() => {
    setTab('body')
  }, [entry?.id])

  return (
    <SidePanel
      title={t('detail.request')}
      tab={tab}
      onTabChange={setTab}
      tabs={tabs}
      bodySize={entry?.requestBody?.length}
      onTitleClick={onTitleClick}>
      <RequestPanelContent tab={tab} entry={entry} t={t} />
    </SidePanel>
  )
}

function RequestPanelContent({ tab, entry, t }: { tab: PanelTab; entry: TrafficEntry | undefined; t: (key: string) => string }) {
  if (!entry) {
    if (tab === 'header' || tab === 'query' || tab === 'cookies' || tab === 'form') {
      return <KeyValueTable data={{}} emptyLabel="" />
    }
    return <Empty><EmptyTitle></EmptyTitle></Empty>
  }

  if (tab === 'header') {
    return <KeyValueTable data={entry.requestHeaders} emptyLabel={t('detail.noHeaders')} />
  }

  if (tab === 'query') {
    const queryData = entry.requestQuery ?? {}
    return <KeyValueTable data={queryData} emptyLabel={t('detail.noQuery')} />
  }

  if (tab === 'cookies') {
    const cookies = parseCookies(entry.requestHeaders)
    return <KeyValueTable data={cookies} emptyLabel={t('detail.noCookies')} />
  }

  if (tab === 'form') {
    const formCt = entry.requestHeaders['content-type'] ?? entry.requestHeaders['Content-Type'] ?? ''
    return <FormDataView body={entry.requestBody ?? ''} contentType={formCt} />
  }

  if (tab === 'body') {
    return entry.requestBody ? (
      <BodyView body={entry.requestBody} contentType={entry.requestContentType} />
    ) : (
      <Empty><EmptyTitle>{t('detail.noRequestBody')}</EmptyTitle></Empty>
    )
  }

  // raw
  const content = formatRequestRaw(entry)
  return <RawView content={content} />
}
