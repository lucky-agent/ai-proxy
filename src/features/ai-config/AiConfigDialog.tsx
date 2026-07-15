import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { PlusIcon, Trash2Icon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { useLocale } from '@/hooks/useLocale'
import type { AiConfig, AiUrlRule } from '@/types/settings'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** provider 下拉里「自动检测」对应后端 provider: null */
const PROVIDER_AUTO = 'auto'

function selectToProvider(value: string): string | null {
  return value === PROVIDER_AUTO ? null : value
}

function providerToSelect(provider: string | null): string {
  return provider ?? PROVIDER_AUTO
}

export default function AiConfigDialog({ open, onOpenChange }: Props) {
  const { t } = useLocale()
  const [enabled, setEnabled] = useState(false)
  const [rules, setRules] = useState<AiUrlRule[]>([])
  const [editingIndex, setEditingIndex] = useState<number | null>(null)
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!open) return
    setLoading(true)
    setError('')
    setEditingIndex(null)
    invoke<AiConfig>('get_ai_config')
      .then((config) => {
        setEnabled(config.enabled)
        setRules(config.detection.url_patterns)
      })
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false))
  }, [open])

  // 点击编辑行之外 → 收起编辑框（仅退出编辑态，不落盘；值仍留在内存，
  // 只有「保存」按钮才写入 setting.json）。放行：编辑行内 & provider 下拉浮层内。
  useEffect(() => {
    if (editingIndex === null) return
    function handlePointerDown(e: PointerEvent) {
      const target = e.target as Element | null
      if (!target) return
      if (target.closest('[data-editing-row]')) return
      if (target.closest('[data-slot="select-content"]')) return
      setEditingIndex(null)
    }
    document.addEventListener('pointerdown', handlePointerDown)
    return () => document.removeEventListener('pointerdown', handlePointerDown)
  }, [editingIndex])

  function toggleItem(index: number) {
    setRules((prev) => {
      const updated = [...prev]
      updated[index] = { ...updated[index], enabled: !updated[index].enabled }
      return updated
    })
  }

  function updateUrl(index: number, url: string) {
    setRules((prev) => {
      const updated = [...prev]
      updated[index] = { ...updated[index], url }
      return updated
    })
  }

  function updateProvider(index: number, value: string) {
    setRules((prev) => {
      const updated = [...prev]
      updated[index] = { ...updated[index], provider: selectToProvider(value) }
      return updated
    })
  }

  /** 追加一条空规则并直接进入编辑态 */
  function addRule() {
    setRules((prev) => {
      const next = [...prev, { url: '', provider: 'openai', enabled: true }]
      setEditingIndex(next.length - 1)
      return next
    })
    setError('')
  }

  function removeItem(index: number) {
    setRules((prev) => prev.filter((_, i) => i !== index))
    setEditingIndex(null)
  }

  async function handleSave() {
    // 重复 URL 校验（忽略空行、大小写）
    const seen = new Set<string>()
    for (const r of rules) {
      const key = r.url.trim().toLowerCase()
      if (!key) continue
      if (seen.has(key)) {
        setError(t('aiConfig.duplicateUrl'))
        return
      }
      seen.add(key)
    }

    setSaving(true)
    setError('')
    // 丢弃空 url 行（等价删除）
    const finalRules = rules.filter((r) => r.url.trim() !== '')
    try {
      await invoke('save_ai_config', {
        ai: { enabled, detection: { url_patterns: finalRules } },
      })
      setRules(finalRules)
      setEditingIndex(null)
      onOpenChange(false)
    } catch (err) {
      setError(String(err))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t('aiConfig.title')}</DialogTitle>
          <DialogDescription>{t('aiConfig.description')}</DialogDescription>
        </DialogHeader>

        {loading ? (
          <p className="text-sm text-muted-foreground">{t('settings.loading')}</p>
        ) : (
          <div className="grid gap-2">
            <label className="flex items-center gap-2 rounded-lg border border-border px-3 py-2 cursor-pointer">
              <Checkbox
                checked={enabled}
                onCheckedChange={(v) => setEnabled(v === true)}
              />
              <span className="text-sm font-medium">{t('aiConfig.enableDetection', 'AI 检测')}</span>
              <span className="ml-auto text-xs text-muted-foreground">
                {enabled ? t('aiConfig.enabledOn', '已开启') : t('aiConfig.enabledOff', '已关闭')}
              </span>
            </label>

            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-muted-foreground">
                {t('aiConfig.ruleListHeader')}
                <span className="ml-2 font-normal opacity-70">
                  {t('aiConfig.editHint')}
                </span>
              </span>
              <Button variant="outline" size="xs" onClick={addRule}>
                <PlusIcon className="size-3.5" />
                {t('aiConfig.addRule')}
              </Button>
            </div>

            {rules.length === 0 ? (
              <p className="text-xs text-muted-foreground px-0.5">
                {t('aiConfig.emptyList')}
              </p>
            ) : (
              <div className="max-h-72 overflow-y-auto rounded-lg border border-border">
                <Table>
                  <TableHeader className="sticky top-0 z-10 bg-card">
                    <TableRow>
                      <TableHead className="h-8 w-10 text-center text-xs">
                        {t('aiConfig.colEnabled')}
                      </TableHead>
                      <TableHead className="h-8 text-xs">
                        {t('aiConfig.colUrl')}
                      </TableHead>
                      <TableHead className="h-8 w-28 text-xs">
                        {t('aiConfig.colProvider')}
                      </TableHead>
                      <TableHead className="h-8 w-9" />
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {rules.map((rule, index) => {
                      const editing = editingIndex === index
                      return (
                        <TableRow
                          key={index}
                          {...(editing ? { 'data-editing-row': '' } : {})}
                        >
                          <TableCell className="py-1 text-center">
                            <Checkbox
                              checked={rule.enabled}
                              onCheckedChange={() => toggleItem(index)}
                            />
                          </TableCell>
                          <TableCell
                            className="py-1"
                            onDoubleClick={() => setEditingIndex(index)}
                          >
                            {editing ? (
                              <Input
                                autoFocus
                                className="h-7 min-w-0 font-mono text-xs"
                                value={rule.url}
                                placeholder={t('aiConfig.placeholderUrl')}
                                onChange={(e) => updateUrl(index, e.target.value)}
                                onKeyDown={(e) => {
                                  if (e.key === 'Enter' || e.key === 'Escape') {
                                    setEditingIndex(null)
                                  }
                                }}
                              />
                            ) : (
                              <span className="block truncate font-mono text-xs">
                                {rule.url || (
                                  <span className="text-muted-foreground italic">
                                    {t('aiConfig.placeholderUrl')}
                                  </span>
                                )}
                              </span>
                            )}
                          </TableCell>
                          <TableCell
                            className="py-1"
                            onDoubleClick={() => setEditingIndex(index)}
                          >
                            {editing ? (
                              <Select
                                value={providerToSelect(rule.provider)}
                                onValueChange={(v) => {
                                  if (v) updateProvider(index, v)
                                }}
                              >
                                <SelectTrigger className="h-7 w-full text-xs">
                                  <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                  <SelectItem value="openai">openai</SelectItem>
                                  <SelectItem value="anthropic">
                                    anthropic
                                  </SelectItem>
                                  <SelectItem value={PROVIDER_AUTO}>
                                    {t('aiConfig.providerAuto')}
                                  </SelectItem>
                                </SelectContent>
                              </Select>
                            ) : (
                              <Badge variant="secondary" className="font-normal">
                                {rule.provider ?? t('aiConfig.providerAuto')}
                              </Badge>
                            )}
                          </TableCell>
                          <TableCell className="py-1 pr-1.5">
                            <button
                              type="button"
                              onClick={() => removeItem(index)}
                              className="inline-flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                              title={t('aiConfig.delete')}
                            >
                              <Trash2Icon className="size-3.5" />
                            </button>
                          </TableCell>
                        </TableRow>
                      )
                    })}
                  </TableBody>
                </Table>
              </div>
            )}
          </div>
        )}

        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('aiConfig.cancel')}
          </Button>
          <Button onClick={handleSave} disabled={loading || saving}>
            {saving ? t('aiConfig.saving') : t('aiConfig.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
