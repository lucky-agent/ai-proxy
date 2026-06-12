import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { CodeIcon, PlusIcon, Trash2Icon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import type { ScriptConfig } from '@/types/settings'

interface Props {
  onOpenFullConfig: () => void
}

export function ScriptToolbar({ onOpenFullConfig }: Props) {
  const { t } = useLocale()
  const [scriptConfig, setScriptConfig] = useState<ScriptConfig>({
    enabled: false,
    scripts: [],
  })
  const [newName, setNewName] = useState('')
  const [newDomain, setNewDomain] = useState('')
  const [addError, setAddError] = useState('')

  useEffect(() => {
    invoke<ScriptConfig>('get_script_config')
      .then((config) => setScriptConfig(config))
      .catch(() => {})
  }, [])

  async function toggleGlobal() {
    const updated = { ...scriptConfig, enabled: !scriptConfig.enabled }
    setScriptConfig(updated)
    try {
      await invoke('save_script_config', { script: updated })
    } catch (_) {}
  }

  async function addScript() {
    const name = newName.trim()
    if (!name) return
    const exists = scriptConfig.scripts.some(
      (item) => item.name.toLowerCase() === name.toLowerCase()
    )
    if (exists) {
      setAddError(t('scriptConfig.duplicateName'))
      return
    }
    const updated: ScriptConfig = {
      ...scriptConfig,
      scripts: [...scriptConfig.scripts, { name, domain: newDomain.trim(), enabled: true }],
    }
    setScriptConfig(updated)
    setNewName('')
    setNewDomain('')
    setAddError('')
    try {
      await invoke('save_script_config', { script: updated })
    } catch (_) {}
  }

  async function removeItem(index: number) {
    const updated: ScriptConfig = {
      ...scriptConfig,
      scripts: scriptConfig.scripts.filter((_, i) => i !== index),
    }
    setScriptConfig(updated)
    try {
      await invoke('save_script_config', { script: updated })
    } catch (_) {}
  }

  async function toggleItem(index: number) {
    const scripts = [...scriptConfig.scripts]
    scripts[index] = { ...scripts[index], enabled: !scripts[index].enabled }
    const updated: ScriptConfig = { ...scriptConfig, scripts }
    setScriptConfig(updated)
    try {
      await invoke('save_script_config', { script: updated })
    } catch (_) {}
  }

  return (
    <div className="flex items-center gap-1.5 min-w-0 overflow-hidden">
      {/* Global toggle */}
      <button
        type="button"
        onClick={toggleGlobal}
        title={t('scriptConfig.globalToggle')}
        className={`inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] font-medium transition-colors ${
          scriptConfig.enabled
            ? 'bg-emerald-500/15 text-emerald-400 hover:bg-emerald-500/25'
            : 'bg-muted text-muted-foreground hover:bg-muted/80'
        }`}
      >
        <CodeIcon className="size-3" />
        {t('scriptConfig.toolbarToggle')}
      </button>

      {/* Script badges */}
      {scriptConfig.scripts.length > 0 && (
        <div className="flex items-center gap-0.5 overflow-hidden">
          {scriptConfig.scripts.map((item, index) => (
            <span
              key={item.name}
              className={`group inline-flex items-center gap-0.5 rounded px-1 py-0.5 text-[11px] transition-colors cursor-default ${
                item.enabled
                  ? 'bg-accent text-accent-foreground'
                  : 'bg-muted/60 text-muted-foreground/60'
              }`}
              onClick={() => toggleItem(index)}
              title={`${item.name}${item.enabled ? '' : ' (disabled)'}`}
            >
              <span className="truncate max-w-[80px]">{item.name}</span>
              {item.domain && (
                <span className="truncate max-w-[60px] text-[10px] text-muted-foreground/70">
                  {item.domain}
                </span>
              )}
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation()
                  removeItem(index)
                }}
                className="hidden group-hover:inline-flex size-3 shrink-0 items-center justify-center rounded text-muted-foreground hover:text-destructive"
                title={t('scriptConfig.delete')}
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
          placeholder={t('scriptConfig.toolbarAddPlaceholder')}
          value={newName}
          onChange={(e) => {
            setNewName(e.target.value)
            setAddError('')
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') addScript()
          }}
        />
        <input
          className="h-5 w-28 rounded border border-border bg-background px-1 text-[11px] outline-none focus-visible:border-ring"
          placeholder={t('scriptConfig.toolbarAddDomain')}
          value={newDomain}
          onChange={(e) => {
            setNewDomain(e.target.value)
            setAddError('')
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') addScript()
          }}
        />
        <button
          type="button"
          onClick={addScript}
          disabled={!newName.trim()}
          className="inline-flex size-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground disabled:opacity-40"
          title={t('scriptConfig.addScript')}
        >
          <PlusIcon className="size-3" />
        </button>
      </div>

      {/* Expand to full config dialog */}
      <button
        type="button"
        onClick={onOpenFullConfig}
        className="inline-flex size-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        title={t('scriptConfig.title')}
      >
        <CodeIcon className="size-3" />
      </button>

      {addError && (
        <span className="text-[11px] text-destructive">{addError}</span>
      )}
    </div>
  )
}
