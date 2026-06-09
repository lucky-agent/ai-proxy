import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Button } from '@/components/ui/button'
import { PlayIcon, SquareIcon, Trash2Icon } from 'lucide-react'
import { TrafficLog } from '@/features/traffic-log'
import { SettingsDialog } from '@/features/settings'
import { AboutDialog } from '@/features/about'
import { TitleBar } from '@/features/title-bar'
import { useProxyEvents } from '@/hooks/useProxyEvents'
import { useTheme } from '@/hooks/useTheme'
import { useLocale } from '@/hooks/useLocale'

function App() {
  const [status, setStatus] = useState('Stopped')
  const [running, setRunning] = useState(false)
  const [error, setError] = useState('')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [aboutOpen, setAboutOpen] = useState(false)
  const { entries, clear } = useProxyEvents()
  const { theme, setTheme } = useTheme()
  const { t } = useLocale()

  useEffect(() => {
    checkStatus()
  }, [])

  useEffect(() => {
    const unsubs: Array<() => void> = []

    listen('open-settings', () => setSettingsOpen(true)).then((unlisten) => {
      unsubs.push(unlisten)
    })

    getCurrentWindow()
      .listen('open-settings', () => setSettingsOpen(true))
      .then((unlisten) => {
        unsubs.push(unlisten)
      })

    return () => {
      unsubs.forEach((unlisten) => unlisten())
    }
  }, [])

  async function checkStatus() {
    try {
      const s = await invoke<string>('get_status')
      if (s.startsWith('Running')) {
        setStatus(s)
        setRunning(true)
      } else {
        setStatus(s)
        setRunning(false)
      }
    } catch (_) {}
  }

  async function startProxy() {
    setError('')
    try {
      const result = await invoke<string>('start_proxy')
      setStatus(result)
      setRunning(true)
    } catch (err) {
      setError(String(err))
    }
  }

  async function stopProxy() {
    setError('')
    try {
      const result = await invoke<string>('stop_proxy')
      setStatus(result)
      setRunning(false)
    } catch (err) {
      setError(String(err))
    }
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-background text-foreground">
      <TitleBar
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenAbout={() => setAboutOpen(true)}
        onOpenTools={() => {}}
      />
      <header className="flex shrink-0 items-center justify-between border-b border-border px-2 py-1">
        <div className="flex items-center gap-2">
          <span
            className={`flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] font-medium ${
              running ? 'bg-emerald-500/10 text-emerald-400' : 'bg-muted text-muted-foreground'
            }`}>
            <span
              className={`inline-block size-1.5 rounded-full ${
                running ? 'bg-emerald-400 animate-pulse' : 'bg-muted-foreground'
              }`}
            />
            {running ? t('app.running') : t('app.stopped')}
          </span>
        </div>
        <div className="flex items-center gap-0.5" data-tauri-drag-region={false}>
          <Button
            variant={running ? 'destructive' : 'default'}
            size="icon-xs"
            onClick={running ? stopProxy : startProxy}>
            {running ? <SquareIcon className="size-3" /> : <PlayIcon className="size-3" />}
          </Button>
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={clear}
            title={t('traffic.clear')}
          >
            <Trash2Icon className="size-3" />
          </Button>
        </div>
      </header>

      {error && (
        <div className="shrink-0 border-b border-destructive/30 bg-destructive/10 px-5 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      <TrafficLog entries={entries} />
      <AboutDialog open={aboutOpen} onOpenChange={setAboutOpen} />
      <SettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        theme={theme}
        onThemeChange={setTheme}
      />
    </div>
  )
}

export default App
