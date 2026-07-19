import { useState } from 'react'
import { useLocale } from '@/hooks/useLocale'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { KeyValuePair, BodyType } from '@/types/collection'
import BodyTab from './tabs/BodyTab'
import AuthTab from './tabs/AuthTab'
import { KeyValueEditor } from './tabs/KeyValueEditor'

type EditorTab = 'params' | 'body' | 'headers' | 'cookies' | 'auth'

const TABS: { id: EditorTab; labelKey: string }[] = [
  { id: 'params', labelKey: 'requestEditor.params' },
  { id: 'body', labelKey: 'requestEditor.body' },
  { id: 'headers', labelKey: 'requestEditor.headers' },
  { id: 'cookies', labelKey: 'requestEditor.cookies' },
  { id: 'auth', labelKey: 'requestEditor.auth' },
]

interface RequestEditorProps {
  params: KeyValuePair[]
  headers: KeyValuePair[]
  cookies: KeyValuePair[]
  body: string
  bodyType: BodyType
  onParamsChange: (params: KeyValuePair[]) => void
  onHeadersChange: (headers: KeyValuePair[]) => void
  onCookiesChange: (cookies: KeyValuePair[]) => void
  onBodyChange: (body: string) => void
  onBodyTypeChange: (bodyType: BodyType) => void
}

export default function RequestEditor(props: RequestEditorProps) {
  const { t } = useLocale()
  const [tab, setTab] = useState<EditorTab>('body')

  return (
    <Tabs value={tab} onValueChange={(v) => setTab(v as EditorTab)} className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <TabsList variant="line" className="shrink-0 justify-start border-b border-surface-elevated bg-surface-elevated/20 px-0 rounded-none">
        {TABS.map(x => (
          <TabsTrigger
            key={x.id}
            value={x.id}
            className="text-ui-sm"
          >
            {t(x.labelKey)}
          </TabsTrigger>
        ))}
      </TabsList>

      <TabsContent value="params" className="min-h-0 flex-1 overflow-hidden mt-0">
        <ScrollArea className="h-full"><KeyValueEditor entries={props.params} onChange={props.onParamsChange} title={t('detail.query')} addLabel={t('requestEditor.addParam')} emptyLabel={t('detail.noQuery')} /></ScrollArea>
      </TabsContent>
      <TabsContent value="body" className="min-h-0 flex-1 overflow-hidden mt-0 flex flex-col">
        <BodyTab body={props.body} bodyType={props.bodyType} onBodyChange={props.onBodyChange} onBodyTypeChange={props.onBodyTypeChange} />
      </TabsContent>
      <TabsContent value="headers" className="min-h-0 flex-1 overflow-hidden mt-0">
        <ScrollArea className="h-full"><KeyValueEditor entries={props.headers} onChange={props.onHeadersChange} title={t('detail.headers')} addLabel={t('requestEditor.addHeader')} emptyLabel={t('detail.noHeaders')} /></ScrollArea>
      </TabsContent>
      <TabsContent value="cookies" className="min-h-0 flex-1 overflow-hidden mt-0">
        <ScrollArea className="h-full"><KeyValueEditor entries={props.cookies} onChange={props.onCookiesChange} title="Cookies" addLabel={t('requestEditor.addCookie')} emptyLabel="No cookies" /></ScrollArea>
      </TabsContent>
      <TabsContent value="auth" className="min-h-0 flex-1 overflow-hidden mt-0">
        <ScrollArea className="h-full"><AuthTab headers={props.headers} onHeadersChange={props.onHeadersChange} /></ScrollArea>
      </TabsContent>
    </Tabs>
  )
}
