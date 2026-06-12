import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { PlusIcon, ShieldCheckIcon, ShieldOffIcon, Trash2Icon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import type { SslConfig } from '@/types/settings'

interface Props {
  onOpenFullConfig: () => void
}

export function SslToolbar({ onOpenFullConfig }: Props) {
  const { t } = useLocale()
  const [sslConfig, setSslConfig] = useState<SslConfig>({
    enabled: false,
    whitelist: [],
  })
  const [newDomain, setNewDomain] = useState('')
  const [addError, setAddError] = useState('')

  useEffect(() => {
    invoke<SslConfig>('get_ssl_config')
      .then((config) => setSslConfig(config))
      .catch(() => {})
  }, [])

  async function toggleGlobal() {
    const updated = { ...sslConfig, enabled: !sslConfig.enabled }
    setSslConfig(updated)
    try {
      await invoke('save_ssl_config', { ssl: updated })
    } catch (_) {}
  }

  async function addDomain() {
    const domain = newDomain.trim()
    if (!domain) return
    const exists = sslConfig.whitelist.some(
      (item) => item.domain.toLowerCase() === domain.toLowerCase()
    )
    if (exists) {
      setAddError(t('sslConfig.duplicateDomain'))
      return
    }
    const updated: SslConfig = {
      ...sslConfig,
      whitelist: [...sslConfig.whitelist, { domain, enabled: true }],
    }
    setSslConfig(updated)
    setNewDomain('')
    setAddError('')
    try {
      await invoke('save_ssl_config', { ssl: updated })
    } catch (_) {}
  }

  async function removeItem(index: number) {
    const updated: SslConfig = {
      ...sslConfig,
      whitelist: sslConfig.whitelist.filter((_, i) => i !== index),
    }
    setSslConfig(updated)
    try {
      await invoke('save_ssl_config', { ssl: updated })
    } catch (_) {}
  }

  async function toggleItem(index: number) {
    const whitelist = [...sslConfig.whitelist]
    whitelist[index] = { ...whitelist[index], enabled: !whitelist[index].enabled }
    const updated: SslConfig = { ...sslConfig, whitelist }
    setSslConfig(updated)
    try {
      await invoke('save_ssl_config', { ssl: updated })
    } catch (_) {}
  }

  return (
    <div className="flex items-center gap-1.5 min-w-0 overflow-hidden">
      {/* Global toggle */}
      <button
        type="button"
        onClick={toggleGlobal}
        title={t('sslConfig.globalToggle')}
        className={`inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] font-medium transition-colors ${
          sslConfig.enabled
            ? 'bg-emerald-500/15 text-emerald-400 hover:bg-emerald-500/25'
            : 'bg-muted text-muted-foreground hover:bg-muted/80'
        }`}
      >
        {sslConfig.enabled ? (
          <ShieldCheckIcon className="size-3" />
        ) : (
          <ShieldOffIcon className="size-3" />
        )}
        {t('sslConfig.toolbarToggle')}
      </button>

      {/* Whitelist badges */}
      {sslConfig.whitelist.length > 0 && (
        <div className="flex items-center gap-0.5 overflow-hidden">
          {sslConfig.whitelist.map((item, index) => (
            <span
              key={item.domain}
              className={`group inline-flex items-center gap-0.5 rounded px-1 py-0.5 text-[11px] transition-colors cursor-default ${
                item.enabled
                  ? 'bg-accent text-accent-foreground'
                  : 'bg-muted/60 text-muted-foreground/60'
              }`}
              onClick={() => toggleItem(index)}
              title={`${item.domain}${item.enabled ? '' : ' (disabled)'}`}
            >
              <span className="truncate max-w-[80px]">{item.domain}</span>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation()
                  removeItem(index)
                }}
                className="hidden group-hover:inline-flex size-3 shrink-0 items-center justify-center rounded text-muted-foreground hover:text-destructive"
                title={t('sslConfig.delete')}
              >
                <Trash2Icon className="size-2.5" />
              </button>
            </span>
          ))}
        </div>
      )}

      {/* Quick-add input */}
      <div className="flex items-center gap-0.5">
        <input
          className="h-5 w-24 rounded border border-border bg-background px-1 text-[11px] outline-none focus-visible:border-ring"
          placeholder={t('sslConfig.toolbarAddPlaceholder')}
          value={newDomain}
          onChange={(e) => {
            setNewDomain(e.target.value)
            setAddError('')
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') addDomain()
          }}
        />
        <button
          type="button"
          onClick={addDomain}
          disabled={!newDomain.trim()}
          className="inline-flex size-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground disabled:opacity-40"
          title={t('sslConfig.addDomain')}
        >
          <PlusIcon className="size-3" />
        </button>
      </div>

      {/* Expand to full config dialog */}
      <button
        type="button"
        onClick={onOpenFullConfig}
        className="inline-flex size-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        title={t('sslConfig.title')}
      >
        <ShieldCheckIcon className="size-3" />
      </button>

      {addError && (
        <span className="text-[11px] text-destructive">{addError}</span>
      )}
    </div>
  )
}
