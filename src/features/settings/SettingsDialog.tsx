import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { MonitorIcon, MoonIcon, SunIcon } from 'lucide-react'
import { Tooltip, TooltipTrigger, TooltipContent } from '@/components/ui/tooltip'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Select as SelectPrimitive } from '@base-ui/react/select'
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
              <Select value={language} onValueChange={(v) => v && setLanguage(v as LocaleSetting)}>
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectPrimitive.Portal>
                  <SelectPrimitive.Positioner side="bottom" sideOffset={4} alignItemWithTrigger={false} collisionAvoidance={{ side: 'none' }} className="isolate z-50">
                    <SelectPrimitive.Popup className="relative isolate z-50 max-h-(--available-height) w-(--anchor-width) min-w-36 origin-(--transform-origin) overflow-x-hidden overflow-y-auto rounded-lg bg-popover text-popover-foreground shadow-md ring-1 ring-foreground/10 duration-100 data-[side=bottom]:slide-in-from-top-2 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95">
                      <SelectPrimitive.List>
                        <SelectItem value="system">{t('settings.languageSystem')}</SelectItem>
                        <SelectItem value="en">{t('settings.languageEn')}</SelectItem>
                        <SelectItem value="zh">{t('settings.languageZh')}</SelectItem>
                      </SelectPrimitive.List>
                    </SelectPrimitive.Popup>
                  </SelectPrimitive.Positioner>
                </SelectPrimitive.Portal>
              </Select>
            </label>
            <div className="grid gap-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t('settings.theme')}
              </span>
              <ToggleGroup
                value={[theme]}
                onValueChange={(value) => {
                  if (value && value.length > 0) onThemeChange(value[0] as Theme)
                }}
                size="sm"
                className="w-fit rounded-md border border-border bg-surface-elevated/50 p-0.5">
                {(
                  [
                    { value: 'light' as Theme, icon: SunIcon },
                    { value: 'dark' as Theme, icon: MoonIcon },
                    { value: 'system' as Theme, icon: MonitorIcon },
                  ] as const
                ).map(({ value, icon: Icon }) => (
                  <Tooltip key={value}>
                    <TooltipTrigger className="inline-flex">
                      <ToggleGroupItem
                        value={value}
                        className="inline-flex size-7 items-center justify-center rounded-[4px] border border-transparent text-muted-foreground transition-colors cursor-pointer hover:text-foreground aria-pressed:border-ring/60 aria-pressed:bg-background aria-pressed:text-foreground aria-pressed:shadow-sm aria-pressed:hover:bg-background"
                      >
                        <Icon className="size-3.5" />
                      </ToggleGroupItem>
                    </TooltipTrigger>
                    <TooltipContent side="top" className="bg-popover text-popover-foreground text-[11px]">
                      {t(`theme.${value}`)}
                    </TooltipContent>
                  </Tooltip>
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
