import { useState, useEffect, useMemo, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { TrafficLog, EditRequestDialog } from '@/features/traffic-log'
import { SettingsDialog } from '@/features/settings'
import { AboutDialog } from '@/features/about'
import { SslConfigDialog } from '@/features/ssl-config'
import { ScriptConfigDialog } from '@/features/script-config'
import { TitleBar } from '@/features/title-bar'
import { ToolBar } from '@/features/tool-bar'
import { BottomBar, type DetailPosition } from '@/features/bottom-bar'
import { AiView } from '@/features/ai-view'
import { NewRequestView } from '@/features/new-request'
import type { ViewId } from '@/types/view'
import { useProxyEvents } from '@/hooks/useProxyEvents'
import { useTheme } from '@/hooks/useTheme'
import { useLocale } from '@/hooks/useLocale'
import { classifyEntry, TYPE_FILTERS, type TypeFilter } from '@/lib/format'
import { TooltipProvider } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'

const TYPE_FILTER_LABELS: Record<TypeFilter, string> = {
  all: 'typeFilter.all',
  http: 'typeFilter.http',
  https: 'typeFilter.https',
  websocket: 'typeFilter.websocket',
  js: 'typeFilter.js',
  css: 'typeFilter.css',
  html: 'typeFilter.html',
  json: 'typeFilter.json',
  img: 'typeFilter.img',
  font: 'typeFilter.font',
  media: 'typeFilter.media',
  other: 'typeFilter.other',
}

function TypeFilterBar({
  active,
  counts,
  onChange,
  running,
  status,
}: {
  active: TypeFilter
  counts: Map<TypeFilter, number>
  onChange: (f: TypeFilter) => void
  running: boolean
  status: string
}) {
  const { t } = useLocale()

  return (
    <div className="flex shrink-0 items-center gap-0.5 overflow-x-auto px-1.5 py-1 border-b border-surface-elevated bg-surface-base/50">
      <div className="flex items-center gap-0.5 overflow-x-auto">
        {TYPE_FILTERS.map(f => {
          const count = counts.get(f) ?? 0
          return (
            <button
              key={f}
              type="button"
              onClick={() => onChange(f)}
              className={cn(
                'relative px-2 py-1 text-[11px] font-medium transition-colors whitespace-nowrap',
                active === f
                  ? 'text-foreground'
                  : 'text-muted-foreground hover:text-foreground'
              )}>
              {t(TYPE_FILTER_LABELS[f])}
              {active === f && (
                <span className="absolute bottom-0 left-1 right-1 h-0.5 bg-foreground/70 rounded-full" />
              )}
              {count > 0 && (
                <span className={cn(
                  'text-[10px] tabular-nums',
                  active === f ? 'text-foreground/50' : 'text-muted-foreground/60'
                )}>
                  {count}
                </span>
              )}
            </button>
          )
        })}
      </div>
      <div className="ml-auto shrink-0" />
      {running ? (
        <span className="flex items-center gap-1.5 rounded-md px-1.5 py-0.5 text-[11px] font-medium bg-emerald-500/10 text-emerald-400 shrink-0">
          <span className="inline-block size-1.5 rounded-full bg-emerald-400 animate-pulse" />
          {status.replace('Running on ', '')}
        </span>
      ) : (
        <span className="flex items-center gap-1.5 rounded-md px-1.5 py-0.5 text-[11px] font-medium bg-muted text-muted-foreground shrink-0">
          <span className="inline-block size-1.5 rounded-full bg-muted-foreground" />
          {t('app.stopped')}
        </span>
      )}
    </div>
  )
}

function App() {
  const [status, setStatus] = useState('Stopped')
  const [running, setRunning] = useState(false)
  const [error, setError] = useState('')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [aboutOpen, setAboutOpen] = useState(false)
  const [sslConfigOpen, setSslConfigOpen] = useState(false)
  const [scriptConfigOpen, setScriptConfigOpen] = useState(false)
  const [sendRequestOpen, setSendRequestOpen] = useState(false)
  const [showDomainSidebar, setShowDomainSidebar] = useState(true)
  const [detailPosition, setDetailPosition] = useState<DetailPosition>('bottom')
  const [scriptEnabled, setScriptEnabled] = useState(false)
  const [sslEnabled, setSslEnabled] = useState(false)
  const [typeFilter, setTypeFilter] = useState<TypeFilter>('all')
  const [activeView, setActiveView] = useState<ViewId>('proxy')
  // mountedViews: which views have their component currently mounted.
  // Closing a tab = unmount (remove from set) + switch to proxy.
  // Clicking a tab = mount (add to set) + switch to it.
  const [mountedViews, setMountedViews] = useState<Set<ViewId>>(new Set(['proxy']))

  const handleCloseTab = useCallback((view: ViewId) => {
    if (view === 'proxy') return
    setMountedViews(prev => {
      const next = new Set(prev)
      next.delete(view)
      return next
    })
    setActiveView('proxy')
  }, [])

  const handleViewChange = useCallback((view: ViewId) => {
    setMountedViews(prev => new Set(prev).add(view))
    setActiveView(view)
  }, [])

  const handleNewRequestSuccess = useCallback((_entryId: string) => {
    // 不跳转，保持在 new-request 视图查看响应
  }, [])
  const { entries, clear } = useProxyEvents()
  const { theme, setTheme } = useTheme()
  const { t } = useLocale()

  const typeCounts = useMemo(() => {
    const counts = new Map<TypeFilter, number>()
    for (const e of entries) {
      const t = classifyEntry(e)
      counts.set(t, (counts.get(t) ?? 0) + 1)
    }
    counts.set('all', entries.length)
    return counts
  }, [entries])

  useEffect(() => {
    checkStatus()
    loadScriptAndSslState()
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

  async function loadScriptAndSslState() {
    try {
      const settings = await invoke<{ ssl: { enabled: boolean }; script: { enabled: boolean } }>('get_settings')
      setSslEnabled(settings.ssl.enabled)
      setScriptEnabled(settings.script.enabled)
    } catch (_) {}
  }

  async function toggleScript() {
    const next = !scriptEnabled
    setScriptEnabled(next)
    try {
      await invoke('save_script_config', { script: { enabled: next, scripts: [] } })
    } catch (_) {}
  }

  async function toggleSsl() {
    const next = !sslEnabled
    setSslEnabled(next)
    try {
      await invoke('save_ssl_config', { ssl: { enabled: next, whitelist: [] } })
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
    <TooltipProvider>
      <div className="flex h-full flex-col overflow-hidden bg-surface-deep text-foreground">
      <TitleBar
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenAbout={() => setAboutOpen(true)}
        onOpenSslConfig={() => setSslConfigOpen(true)}
        onOpenScriptConfig={() => setScriptConfigOpen(true)}
        onOpenSendRequest={() => setSendRequestOpen(true)}
        running={running}
        onStartProxy={startProxy}
        onStopProxy={stopProxy}
        onClearTraffic={clear}
        activeView={activeView}
        mountedViews={mountedViews}
        onViewChange={handleViewChange}
        onCloseTab={handleCloseTab}
      />

      <div className="flex min-h-0 flex-1 overflow-hidden">
        <ToolBar activeView={activeView} mountedViews={mountedViews} onViewChange={handleViewChange} />

        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          {mountedViews.has('proxy') && activeView === 'proxy' && (
            <>
              <TypeFilterBar active={typeFilter} counts={typeCounts} onChange={setTypeFilter} running={running} status={status} />
              {error && (
                <div className="shrink-0 border-b border-destructive/30 bg-destructive/10 px-5 py-2 text-sm text-destructive">
                  {error}
                </div>
              )}
              <TrafficLog entries={entries} showDomainSidebar={showDomainSidebar} detailPosition={detailPosition} onAutoOpenDetail={() => setDetailPosition('bottom')} typeFilter={typeFilter} />
            </>
          )}
          {mountedViews.has('new-request') && activeView === 'new-request' && <NewRequestView onSendSuccess={handleNewRequestSuccess} entries={entries} />}
          {mountedViews.has('ai') && activeView === 'ai' && <AiView />}
        </div>
      </div>

      <BottomBar
        showDomainSidebar={showDomainSidebar}
        onToggleDomainSidebar={() => setShowDomainSidebar(v => !v)}
        detailPosition={detailPosition}
        onToggleDetailPosition={setDetailPosition}
        scriptEnabled={scriptEnabled}
        onToggleScript={toggleScript}
        sslEnabled={sslEnabled}
        onToggleSsl={toggleSsl}
      />

      <AboutDialog open={aboutOpen} onOpenChange={setAboutOpen} />
      <SslConfigDialog open={sslConfigOpen} onOpenChange={setSslConfigOpen} />
      <ScriptConfigDialog open={scriptConfigOpen} onOpenChange={setScriptConfigOpen} />
      <EditRequestDialog
        open={sendRequestOpen}
        onOpenChange={setSendRequestOpen}
        entry={null}
      />
      <SettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        theme={theme}
        onThemeChange={setTheme}
      />
    </div>
    </TooltipProvider>
  )
}

export default App
