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
import { useLocale } from '@/hooks/useLocale'
import type { SslConfig, SslWhitelistItem } from '@/types/settings'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
}

const inputClass =
  'h-auto w-full'

export default function SslConfigDialog({ open, onOpenChange }: Props) {
  const { t } = useLocale()
  const [sslConfig, setSslConfig] = useState<SslConfig>({
    enabled: false,
    whitelist: [],
  })
  const [newDomain, setNewDomain] = useState('')
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!open) return
    setLoading(true)
    setError('')
    invoke<SslConfig>('get_ssl_config')
      .then((config) => setSslConfig(config))
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false))
  }, [open])

  function toggleGlobal() {
    setSslConfig((prev) => ({ ...prev, enabled: !prev.enabled }))
  }

  function toggleItem(index: number) {
    setSslConfig((prev) => {
      const updated = [...prev.whitelist]
      updated[index] = { ...updated[index], enabled: !updated[index].enabled }
      return { ...prev, whitelist: updated }
    })
  }

  function addDomain() {
    const domain = newDomain.trim()
    if (!domain) return
    const exists = sslConfig.whitelist.some(
      (item) => item.domain.toLowerCase() === domain.toLowerCase()
    )
    if (exists) {
      setError(t('sslConfig.duplicateDomain'))
      return
    }
    setSslConfig((prev) => ({
      ...prev,
      whitelist: [...prev.whitelist, { domain, enabled: true }],
    }))
    setNewDomain('')
    setError('')
  }

  function removeItem(index: number) {
    setSslConfig((prev) => ({
      ...prev,
      whitelist: prev.whitelist.filter((_, i) => i !== index),
    }))
  }

  async function handleSave() {
    setSaving(true)
    setError('')
    try {
      await invoke('save_ssl_config', { ssl: sslConfig })
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
          <DialogTitle>{t('sslConfig.title')}</DialogTitle>
          <DialogDescription>{t('sslConfig.description')}</DialogDescription>
        </DialogHeader>

        {loading ? (
          <p className="text-sm text-muted-foreground">{t('settings.loading')}</p>
        ) : (
          <div className="grid gap-4">
            <div className="flex items-center gap-2">
              <Checkbox
                id="ssl-global-toggle"
                checked={sslConfig.enabled}
                onCheckedChange={toggleGlobal}
              />
              <Label htmlFor="ssl-global-toggle" className="font-medium">{t('sslConfig.globalToggle')}</Label>
            </div>

            <div className="grid gap-2">
              <span className="text-xs font-medium text-muted-foreground">
                {t('sslConfig.whitelistHeader')}
              </span>

              <div className="flex gap-2">
                <Input
                  className={inputClass}
                  placeholder={t('sslConfig.placeholderDomain')}
                  value={newDomain}
                  onChange={(e) => setNewDomain(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') addDomain()
                  }}
                />
                <Button
                  variant="outline"
                  size="icon-xs"
                  onClick={addDomain}
                  title={t('sslConfig.addDomain')}
                >
                  <PlusIcon className="size-3.5" />
                </Button>
              </div>

              {sslConfig.whitelist.length === 0 ? (
                <p className="text-xs text-muted-foreground px-0.5">
                  {t('sslConfig.emptyWhitelist')}
                </p>
              ) : (
                <div className="max-h-48 overflow-y-auto rounded-lg border border-border">
                  {sslConfig.whitelist.map((item, index) => (
                    <div
                      key={item.domain}
                      className="flex items-center gap-2 border-b border-border px-2.5 py-1.5 last:border-b-0"
                    >
                      <Checkbox
                        checked={item.enabled}
                        onCheckedChange={() => toggleItem(index)}
                      />
                      <span className="min-w-0 flex-1 truncate text-sm">
                        {item.domain}
                      </span>
                      <button
                        type="button"
                        onClick={() => removeItem(index)}
                        className="inline-flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                        title={t('sslConfig.delete')}
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

        {error && <p className="text-sm text-destructive">{error}</p>}

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('sslConfig.cancel')}
          </Button>
          <Button onClick={handleSave} disabled={loading || saving}>
            {saving ? t('sslConfig.saving') : t('sslConfig.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
