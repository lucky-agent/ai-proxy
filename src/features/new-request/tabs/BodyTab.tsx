import { useLocale } from '@/hooks/useLocale'
import { Textarea } from '@/components/ui/textarea'
import type { BodyType } from '@/types/collection'

interface BodyTabProps {
  body: string
  bodyType: BodyType
  onBodyChange: (body: string) => void
  onBodyTypeChange: (bodyType: BodyType) => void
}

const BODY_FORMATS: { value: BodyType; labelKey: string }[] = [
  { value: 'json', labelKey: 'requestEditor.bodyFormatJson' },
  { value: 'xml', labelKey: 'requestEditor.bodyFormatXml' },
  { value: 'text', labelKey: 'requestEditor.bodyFormatText' },
  { value: 'urlencoded', labelKey: 'requestEditor.bodyFormatUrlencoded' },
  { value: 'multipart', labelKey: 'requestEditor.bodyFormatMultipart' },
]

export default function BodyTab({ body, bodyType, onBodyChange, onBodyTypeChange }: BodyTabProps) {
  const { t } = useLocale()

  return (
    <div className="p-4 space-y-1 min-h-0 flex flex-col flex-1">
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
          {t('detail.body')}
        </span>
        <select
          value={bodyType}
          onChange={e => onBodyTypeChange(e.target.value as BodyType)}
          className="rounded border border-input bg-background px-2 py-1 text-[11px] text-foreground outline-none focus:ring-1 focus:ring-primary"
          title={t('requestEditor.bodyFormat')}
        >
          {BODY_FORMATS.map(fmt => (
            <option key={fmt.value} value={fmt.value}>{t(fmt.labelKey)}</option>
          ))}
        </select>
      </div>
      <Textarea
        value={body}
        onChange={e => onBodyChange(e.target.value)}
        className="flex-1 min-h-[180px] text-xs font-mono resize-y"
        placeholder={
          bodyType === 'json'
            ? '{ "key": "value" }'
            : bodyType === 'xml'
            ? '<root>\n  <key>value</key>\n</root>'
            : bodyType === 'urlencoded'
            ? 'key1=value1&key2=value2'
            : bodyType === 'multipart'
            ? '--boundary'
            : 'body content'
        }
      />
    </div>
  )
}
