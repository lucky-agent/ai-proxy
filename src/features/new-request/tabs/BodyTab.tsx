import { useLocale } from '@/hooks/useLocale'
import CodeEditor from '@/components/code-editor/CodeEditor'
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
    <div className="p-4 min-h-0 flex flex-col flex-1">
      <div className="flex-1 min-h-[180px] border rounded-md overflow-hidden relative group/body">
        <div className="absolute top-1.5 right-1.5 z-10 opacity-0 group-hover/body:opacity-100 transition-all">
          <select
            value={bodyType}
            onChange={e => onBodyTypeChange(e.target.value as BodyType)}
            className="appearance-none rounded bg-surface-elevated/30 px-1.5 py-0.5 text-ui-xs text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50 transition-colors cursor-pointer outline-none border border-surface-elevated/30"
          >
            {BODY_FORMATS.map(fmt => (
              <option key={fmt.value} value={fmt.value}>{t(fmt.labelKey)}</option>
            ))}
          </select>
        </div>
        <CodeEditor value={body} language={bodyType} onChange={onBodyChange} />
      </div>
    </div>
  )
}
