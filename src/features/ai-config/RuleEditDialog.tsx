import { useEffect, useState } from 'react'
import { PlusIcon, XIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useLocale } from '@/hooks/useLocale'
import { Tooltip, TooltipTrigger, TooltipContent } from '@/components/ui/tooltip'
import type { AiRuleSource, AiUrlRule } from '@/types/settings'

/** provider 下拉里「自动检测」对应后端 provider: null */
const PROVIDER_AUTO = 'auto'

/** 「来源名」输入的内置客户端建议（datalist，可自由输入其他名称） */
const BUILTIN_SOURCES = [
  'Claude Code',
  'Cursor',
  'Cline',
  'Windsurf',
  'Cherry Studio',
  'NextChat',
  'LobeChat',
  'Browser',
]
const SOURCE_DATALIST_ID = 'ai-rule-source-names'

interface Props {
  /** null = 新增；否则为被编辑规则（用于表单预填） */
  initial: AiUrlRule | null
  /** 其余规则的 URL（已小写化，重复校验用） */
  existingUrls: string[]
  onConfirm: (rule: Omit<AiUrlRule, 'enabled'>) => void
  onCancel: () => void
}

/** 纯表单组件 — 不含 Dialog 外壳，由父组件嵌入侧滑面板使用 */
export default function RuleEditForm({
  initial,
  existingUrls,
  onConfirm,
  onCancel,
}: Props) {
  const { t } = useLocale()
  const [url, setUrl] = useState('')
  const [provider, setProvider] = useState(PROVIDER_AUTO)
  const [sources, setSources] = useState<AiRuleSource[]>([])
  const [error, setError] = useState('')

  useEffect(() => {
    setUrl(initial?.url ?? '')
    setProvider(initial?.provider ?? (initial ? PROVIDER_AUTO : 'openai'))
    setSources(initial?.sources ?? [])
    setError('')
  }, [initial])

  function updateSource(index: number, patch: Partial<AiRuleSource>) {
    setSources((prev) => prev.map((s, i) => (i === index ? { ...s, ...patch } : s)))
  }

  function addSource() {
    setSources((prev) => [...prev, { name: '', merge_header: '' }])
  }

  function removeSource(index: number) {
    setSources((prev) => prev.filter((_, i) => i !== index))
  }

  function handleConfirm() {
    const trimmed = url.trim()
    if (!trimmed) return
    if (existingUrls.includes(trimmed.toLowerCase())) {
      setError(t('aiConfig.duplicateUrl'))
      return
    }
    const cleaned = sources
      .map((s) => ({ name: s.name.trim(), merge_header: s.merge_header.trim() }))
      .filter((s) => s.name !== '' || s.merge_header !== '')
    onConfirm({
      url: trimmed,
      provider: provider === PROVIDER_AUTO ? null : provider,
      sources: cleaned,
    })
  }

  const titleKey = initial ? 'aiConfig.editRule' : 'aiConfig.addRule'

  return (
    <div className="flex h-full flex-col">
      {/* 标题栏：返回按钮 + 标题 */}
      <div className="flex items-center gap-2 pb-4">
        <Button variant="ghost" size="icon-sm" onClick={onCancel}>
          <XIcon className="size-4" />
          <span className="sr-only">{t('aiConfig.back')}</span>
        </Button>
        <h3 className="text-sm font-medium text-foreground">{t(titleKey)}</h3>
      </div>

      <div className="flex-1 space-y-3 overflow-y-auto">
        {/* URL */}
        <label className="grid gap-1.5">
          <span className="text-xs font-medium text-muted-foreground">
            {t('aiConfig.fieldUrl')}
          </span>
          <Input
            autoFocus
            className="font-mono text-xs"
            value={url}
            placeholder={t('aiConfig.placeholderUrl')}
            onChange={(e) => {
              setUrl(e.target.value)
              setError('')
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleConfirm()
            }}
          />
          {error ? (
            <span className="text-[11px] text-destructive">{error}</span>
          ) : (
            <span className="text-[11px] text-muted-foreground/75">
              {t('aiConfig.urlHint')}
            </span>
          )}
        </label>

        {/* Provider */}
        <label className="grid gap-1.5">
          <span className="text-xs font-medium text-muted-foreground">
            {t('aiConfig.fieldProvider')}
          </span>
          <Select value={provider} onValueChange={(v) => v && setProvider(v)}>
            <SelectTrigger className="w-full">
              <SelectValue>
                {(v: unknown) =>
                  v === PROVIDER_AUTO ? t('aiConfig.providerAuto') : String(v)
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="openai">openai</SelectItem>
              <SelectItem value="anthropic">anthropic</SelectItem>
              <SelectItem value={PROVIDER_AUTO}>
                {t('aiConfig.providerAuto')}
              </SelectItem>
            </SelectContent>
          </Select>
        </label>

        {/* Sources */}
        <div className="grid gap-1.5">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-muted-foreground">
              {t('aiConfig.fieldSources')}
              <span className="ml-1.5 font-normal opacity-70">
                {t('aiConfig.sourcesHint')}
              </span>
            </span>
            <Button variant="ghost" size="xs" onClick={addSource}>
              <PlusIcon className="size-3.5" />
              {t('aiConfig.addSource')}
            </Button>
          </div>
          {sources.length === 0 ? (
            <p className="text-[11px] text-muted-foreground/75">
              {t('aiConfig.sourcesEmpty')}
            </p>
          ) : (
            <div className="grid gap-1.5">
              {sources.map((s, i) => (
                <div key={i} className="flex items-center gap-1.5">
                  <Input
                    className="flex-1 text-xs"
                    list={SOURCE_DATALIST_ID}
                    value={s.name}
                    placeholder={t('aiConfig.sourceNamePlaceholder')}
                    onChange={(e) => updateSource(i, { name: e.target.value })}
                  />
                  <Input
                    className="flex-1 font-mono text-xs"
                    value={s.merge_header}
                    placeholder={t('aiConfig.sourceHeaderPlaceholder')}
                    onChange={(e) =>
                      updateSource(i, { merge_header: e.target.value })
                    }
                  />
                  <Tooltip>
                    <TooltipTrigger className="inline-flex">
                      <button
                        type="button"
                        className="inline-flex size-[22px] shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                        onClick={() => removeSource(i)}
                      >
                        <XIcon className="size-3.5" />
                      </button>
                    </TooltipTrigger>
                    <TooltipContent side="top" className="bg-popover text-popover-foreground text-[11px]">
                      {t('aiConfig.delete')}
                    </TooltipContent>
                  </Tooltip>
                </div>
              ))}
            </div>
          )}
          <datalist id={SOURCE_DATALIST_ID}>
            {BUILTIN_SOURCES.map((name) => (
              <option key={name} value={name} />
            ))}
          </datalist>
        </div>
      </div>

      {/* 底部按钮 */}
      <div className="-mx-4 -mb-4 mt-4 flex items-center justify-end gap-2 rounded-b-xl border-t bg-muted/50 px-4 py-3">
        <Button variant="outline" size="sm" onClick={onCancel}>
          {t('aiConfig.cancel')}
        </Button>
        <Button size="sm" onClick={handleConfirm} disabled={!url.trim()}>
          {t('aiConfig.confirm')}
        </Button>
      </div>
    </div>
  )
}
