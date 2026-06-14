import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { MonitorIcon, MoonIcon, SunIcon } from 'lucide-react'
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
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import type { Theme } from '@/hooks/useTheme'
import type { LocaleSetting } from '@/i18n'
import type { ProxyConfig } from '@/types/settings'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  theme: Theme
  onThemeChange: (theme: Theme) => void
}

const inputClass =
  'h-auto w-full'

const selectClass =
  'h-auto w-full rounded-lg border border-border bg-background px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50'

export default function SettingsDialog({ open, onOpenChange, theme, onThemeChange }: Props) {
  const { t, setLocale } = useLocale()
  const [proxy, setProxy] = useState<ProxyConfig>({
    listen_host: '127.0.0.1',
    listen_port: 5201,
    upstream_proxy: false,
  })
  const [language, setLanguage] = useState<LocaleSetting>('system')
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!open) return
    setLoading(true)
    setError('')
    Promise.all([
      invoke<{ proxy: ProxyConfig }>('get_settings'),
      invoke<LocaleSetting>('get_locale'),
    ])
      .then(([settings, locale]) => {
        setProxy(settings.proxy)
        setLanguage(locale)
      })
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false))
  }, [open])

  async function handleSave() {
    setSaving(true)
    setError('')
    try {
      await invoke('save_settings', { proxy })
      setLocale(language)
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
          <DialogTitle>{t('settings.title')}</DialogTitle>
          <DialogDescription>{t('settings.description')}</DialogDescription>
        </DialogHeader>

        {loading ? (
          <p className="text-sm text-muted-foreground">{t('settings.loading')}</p>
        ) : (
          <div className="grid gap-3">
            <label className="grid gap-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t('settings.language')}
              </span>
              <select
                className={selectClass}
                value={language}
                onChange={(e) => setLanguage(e.target.value as LocaleSetting)}>
                <option value="system">{t('settings.languageSystem')}</option>
                <option value="en">{t('settings.languageEn')}</option>
                <option value="zh">{t('settings.languageZh')}</option>
              </select>
            </label>
            <div className="grid gap-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t('settings.theme')}
              </span>
              <ToggleGroup
                type="single"
                value={theme}
                onValueChange={(value) => {
                  if (value) onThemeChange(value as Theme)
                }}
                variant="outline"
                size="sm"
                className="w-fit rounded-md border border-border bg-surface-elevated/50 p-0.5">
                {(
                  [
                    { value: 'light' as Theme, icon: SunIcon },
                    { value: 'dark' as Theme, icon: MoonIcon },
                    { value: 'system' as Theme, icon: MonitorIcon },
                  ] as const
                ).map(({ value, icon: Icon }) => (
                  <ToggleGroupItem
                    key={value}
                    value={value}
                    className="inline-flex size-7 items-center justify-center rounded-[4px] transition-colors cursor-pointer"
                    title={t(`theme.${value}`)}>
                    <Icon className="size-3.5" />
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
            </div>
            <label className="grid gap-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t('settings.listenHost')}
              </span>
              <Input
                className={inputClass}
                value={proxy.listen_host}
                onChange={(e) => setProxy((p) => ({ ...p, listen_host: e.target.value }))}
              />
            </label>
            <label className="grid gap-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t('settings.listenPort')}
              </span>
              <Input
                className={inputClass}
                type="number"
                min={1}
                max={65535}
                value={proxy.listen_port}
                onChange={(e) =>
                  setProxy((p) => ({ ...p, listen_port: Number(e.target.value) || 0 }))
                }
              />
            </label>
            <label className="flex items-center gap-2 text-sm">
              <Checkbox
                id="upstream-proxy"
                checked={proxy.upstream_proxy}
                onCheckedChange={(checked) => setProxy((p) => ({ ...p, upstream_proxy: !!checked }))}
              />
              <Label htmlFor="upstream-proxy">{t('settings.upstreamProxy')}</Label>
            </label>
          </div>
        )}

        {error && <Alert variant="destructive"><AlertDescription>{error}</AlertDescription></Alert>}

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('settings.cancel')}
          </Button>
          <Button onClick={handleSave} disabled={loading || saving}>
            {saving ? t('settings.saving') : t('settings.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
