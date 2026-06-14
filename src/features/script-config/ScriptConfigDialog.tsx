import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { PlusIcon, Trash2Icon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
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
import type { ScriptConfig, ScriptItem } from '@/types/settings'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
}

const inputClass =
  'h-auto w-full'

export default function ScriptConfigDialog({ open, onOpenChange }: Props) {
  const { t } = useLocale()
  const [scriptConfig, setScriptConfig] = useState<ScriptConfig>({
    enabled: false,
    scripts: [],
  })
  const [newName, setNewName] = useState('')
  const [newDomain, setNewDomain] = useState('')
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!open) return
    setLoading(true)
    setError('')
    invoke<ScriptConfig>('get_script_config')
      .then((config) => setScriptConfig(config))
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false))
  }, [open])

  function toggleGlobal() {
    setScriptConfig((prev) => ({ ...prev, enabled: !prev.enabled }))
  }

  function toggleItem(index: number) {
    setScriptConfig((prev) => {
      const updated = [...prev.scripts]
      updated[index] = { ...updated[index], enabled: !updated[index].enabled }
      return { ...prev, scripts: updated }
    })
  }

  function addScript() {
    const name = newName.trim()
    if (!name) return
    const exists = scriptConfig.scripts.some(
      (item) => item.name.toLowerCase() === name.toLowerCase()
    )
    if (exists) {
      setError(t('scriptConfig.duplicateName'))
      return
    }
    setScriptConfig((prev) => ({
      ...prev,
      scripts: [...prev.scripts, { name, domain: newDomain.trim(), enabled: true }],
    }))
    setNewName('')
    setNewDomain('')
    setError('')
  }

  function removeItem(index: number) {
    setScriptConfig((prev) => ({
      ...prev,
      scripts: prev.scripts.filter((_, i) => i !== index),
    }))
  }

  async function handleSave() {
    setSaving(true)
    setError('')
    try {
      await invoke('save_script_config', { script: scriptConfig })
      onOpenChange(false)
    } catch (err) {
      setError(String(err))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t('scriptConfig.title')}</DialogTitle>
          <DialogDescription>{t('scriptConfig.description')}</DialogDescription>
        </DialogHeader>

        {loading ? (
          <p className="text-sm text-muted-foreground">{t('settings.loading')}</p>
        ) : (
          <div className="grid gap-4">
            <div className="flex items-center gap-2">
              <Checkbox
                id="script-global-toggle"
                checked={scriptConfig.enabled}
                onCheckedChange={toggleGlobal}
              />
              <Label htmlFor="script-global-toggle" className="font-medium">{t('scriptConfig.globalToggle')}</Label>
            </div>

            <div className="grid gap-2">
              <span className="text-xs font-medium text-muted-foreground">
                {t('scriptConfig.scriptListHeader')}
              </span>

              <div className="grid grid-cols-2 gap-2">
                <Input
                  className={inputClass}
                  placeholder={t('scriptConfig.placeholderName')}
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') addScript()
                  }}
                />
                <Input
                  className={inputClass}
                  placeholder={t('scriptConfig.placeholderDomain')}
                  value={newDomain}
                  onChange={(e) => setNewDomain(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') addScript()
                  }}
                />
                <Button
                  variant="outline"
                  size="icon-xs"
                  onClick={addScript}
                  title={t('scriptConfig.addScript')}
                >
                  <PlusIcon className="size-3.5" />
                </Button>
              </div>

              {scriptConfig.scripts.length === 0 ? (
                <p className="text-xs text-muted-foreground px-0.5">
                  {t('scriptConfig.emptyList')}
                </p>
              ) : (
                <div className="max-h-48 overflow-y-auto rounded-lg border border-border">
                  {scriptConfig.scripts.map((item, index) => (
                    <div
                      key={item.name}
                      className="flex items-center gap-2 border-b border-border px-2.5 py-1.5 last:border-b-0"
                    >
                      <Checkbox
                        checked={item.enabled}
                        onCheckedChange={() => toggleItem(index)}
                      />
                      <span className="min-w-0 flex-1 truncate text-sm">
                        {item.name}
                      </span>
                      <span className="text-xs text-muted-foreground truncate max-w-[120px]" title={item.domain}>
                        {item.domain || '—'}
                      </span>
                      <button
                        type="button"
                        onClick={() => removeItem(index)}
                        className="inline-flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                        title={t('scriptConfig.delete')}
                      >
                        <Trash2Icon className="size-3.5" />
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}

        {error && <Alert variant="destructive"><AlertDescription>{error}</AlertDescription></Alert>}

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('scriptConfig.cancel')}
          </Button>
          <Button onClick={handleSave} disabled={loading || saving}>
            {saving ? t('scriptConfig.saving') : t('scriptConfig.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
