import { useState, useEffect, useMemo, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { EditRequestDialog } from '@/features/proxy'
import { SettingsDialog } from '@/features/settings'
import { AboutDialog } from '@/features/about'
import { SslConfigDialog } from '@/features/ssl-config'
import { ScriptConfigDialog } from '@/features/script-config'
import { TitleBar } from '@/features/title-bar'
import { ToolBar } from '@/features/tool-bar'
import { BottomBar, type DetailPosition } from '@/features/bottom-bar'
import { AiView } from '@/features/ai-view'
import { NewRequestView } from '@/features/new-request'
import { ProxyView } from '@/features/proxy'
import type { ViewId } from '@/types/view'
import { useProxyEvents } from '@/hooks/useProxyEvents'
import { useTheme } from '@/hooks/useTheme'
import { classifyEntry, type TypeFilter } from '@/lib/format'
import { TooltipProvider } from '@/components/ui/tooltip'

function App() {
  const [status, setStatus] = useState('Stopped')
  const [running, setRunning] = useState(false)
  const [error, setError] = useState('')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [aboutOpen, setAboutOpen] = useState(false)
  const [sslConfigOpen, setSslConfigOpen] = useState(false)
  const [scriptConfigOpen, setScriptConfigOpen] = useState(false)
  const [sendRequestOpen, setSendRequestOpen] = useState(false)
  const [showSidebar, setShowSidebar] = useState(true)
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
            <ProxyView
              entries={entries}
              error={error}
              showSidebar={showSidebar}
              detailPosition={detailPosition}
              onAutoOpenDetail={() => setDetailPosition('bottom')}
              typeFilter={typeFilter}
              typeCounts={typeCounts}
              onTypeFilterChange={setTypeFilter}
              running={running}
              status={status}
            />
          )}
          {mountedViews.has('new-request') && activeView === 'new-request' && (
              <NewRequestView
                onSendSuccess={handleNewRequestSuccess}
                entries={entries}
                showSidebar={showSidebar}
                detailPosition={detailPosition}
              />
            )}
            {mountedViews.has('ai') && activeView === 'ai' && (
              <AiView showSidebar={showSidebar} detailPosition={detailPosition} />
            )}
        </div>
      </div>

      <BottomBar
        showSidebar={showSidebar}
        onToggleSidebar={() => setShowSidebar(v => !v)}
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
