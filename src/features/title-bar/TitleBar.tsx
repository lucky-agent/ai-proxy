import { useEffect, useRef, useState, type MouseEvent, type ReactNode } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  AlignJustifyIcon,
  InfoIcon,
  LogOutIcon,
  MinusIcon,
  PlayIcon,
  SettingsIcon,
  ShieldCheckIcon,
  SquareIcon,
  SquarePenIcon,
  Trash2Icon,
  XIcon,
  CodeIcon,
} from 'lucide-react'
import appIcon from '@/assets/app-icon.png'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'
import type { ViewId } from '@/types/view'
import { TabBar } from './TabBar'

type TitleBarProps = {
  onOpenSettings: () => void
  onOpenAbout: () => void
  onOpenSslConfig: () => void
  onOpenScriptConfig: () => void
  onOpenSendRequest: () => void
  running: boolean
  onStartProxy: () => void
  onStopProxy: () => void
  onClearTraffic: () => void
  activeView: ViewId
  mountedViews: Set<ViewId>
  onViewChange: (view: ViewId) => void
  onCloseTab: (view: ViewId) => void
}

function stopTitleBarDrag(event: { stopPropagation: () => void }) {
  event.stopPropagation()
}

function TitleBarMenu({
  label,
  children,
}: {
  label: string
  children: (close: () => void) => ReactNode
}) {
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  const closeTimerRef = useRef<ReturnType<typeof setTimeout>>(null)

  function scheduleClose() {
    closeTimerRef.current = setTimeout(() => setOpen(false), 150)
  }

  function cancelClose() {
    if (closeTimerRef.current) {
      clearTimeout(closeTimerRef.current)
      closeTimerRef.current = null
    }
  }

  function close() {
    cancelClose()
    setOpen(false)
  }

  // 菜单打开时，点击外部关闭
  useEffect(() => {
    if (!open) return

    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        close()
      }
    }

    document.addEventListener('pointerdown', onPointerDown, true)
    return () => document.removeEventListener('pointerdown', onPointerDown, true)
  }, [open])

  return (
    <div
      ref={rootRef}
      className="relative h-full"
      data-tauri-drag-region={false}
      onMouseEnter={() => { cancelClose(); setOpen(true) }}
      onMouseLeave={scheduleClose}>
      <button
        type="button"
        data-tauri-drag-region={false}
        aria-expanded={open}
        aria-haspopup="menu"
        onMouseDown={stopTitleBarDrag}
        onPointerDown={stopTitleBarDrag}
        className={cn(
          'inline-flex h-8 items-center rounded-none border-0 px-2.5 text-xs text-foreground/80 shadow-none outline-none transition-colors hover:bg-surface-elevated/50 hover:text-foreground focus:outline-none focus-visible:outline-none',
          open && 'bg-surface-elevated/50 text-foreground'
        )}>
        {label}
      </button>
      {open ? (
        <div
          role="menu"
          data-tauri-drag-region={false}
          className="absolute top-full left-0 z-[200] min-w-32 rounded-lg bg-popover p-1 text-popover-foreground shadow-md ring-1 ring-foreground/10"
          onMouseEnter={cancelClose}
          onMouseLeave={scheduleClose}>
          {children(close)}
        </div>
      ) : null}
    </div>
  )
}

function MenuItem({
  children,
  onClick,
  variant = 'default',
}: {
  children: ReactNode
  onClick: () => void
  variant?: 'default' | 'destructive'
}) {
  return (
    <button
      type="button"
      role="menuitem"
      data-tauri-drag-region={false}
      onMouseDown={stopTitleBarDrag}
      onPointerDown={stopTitleBarDrag}
      onClick={onClick}
      className={cn(
        'flex w-full items-center gap-1.5 rounded-md px-1.5 py-1 text-left text-sm outline-none transition-colors hover:bg-accent hover:text-accent-foreground',
        variant === 'destructive' &&
          'text-destructive hover:bg-destructive/10 hover:text-destructive'
      )}>
      {children}
    </button>
  )
}

function WindowButton({
  label,
  onClick,
  className,
  children,
}: {
  label: string
  onClick: () => void
  className?: string
  children: ReactNode
}) {
  return (
    <button
      type="button"
      aria-label={label}
      data-tauri-drag-region={false}
      onMouseDown={stopTitleBarDrag}
      onPointerDown={stopTitleBarDrag}
      onClick={onClick}
      className={cn(
        'inline-flex h-8 w-11 items-center justify-center text-foreground/70 outline-none transition-colors hover:bg-surface-elevated/50 hover:text-foreground focus:outline-none focus-visible:outline-none',
        className
      )}>
      {children}
    </button>
  )
}
export function TitleBar({ onOpenSettings, onOpenAbout, onOpenSslConfig, onOpenScriptConfig, onOpenSendRequest, running, onStartProxy, onStopProxy, onClearTraffic, activeView, mountedViews, onViewChange, onCloseTab }: TitleBarProps) {
  const { t } = useLocale()
  const appWindow = getCurrentWindow()
  const [toolbarExpanded, setToolbarExpanded] = useState(false)
  const toolbarRef = useRef<HTMLDivElement>(null)

  // 点击外部区域时收起工具栏
  useEffect(() => {
    if (!toolbarExpanded) return

    const onPointerDown = (event: PointerEvent) => {
      if (!toolbarRef.current?.contains(event.target as Node)) {
        setToolbarExpanded(false)
      }
    }

    document.addEventListener('pointerdown', onPointerDown, true)
    return () => document.removeEventListener('pointerdown', onPointerDown, true)
  }, [toolbarExpanded])

  async function handleQuit() {
    await appWindow?.close()
  }

  function handleTitleBarMouseDown(event: MouseEvent<HTMLDivElement>) {
    const target = event.target as HTMLElement
    if (target.closest('[data-tauri-drag-region="false"]')) return
    if (event.button !== 0) return

    if (event.detail === 2) {
      void appWindow?.toggleMaximize()
      return
    }

    void appWindow?.startDragging()
  }

  return (
    <div
      className="titlebar relative flex h-8 shrink-0 items-center border-b border-surface-elevated bg-surface-base select-none"
      data-tauri-drag-region
      onMouseDown={handleTitleBarMouseDown}>
      <img src={appIcon} alt="" className="ml-2 size-4 shrink-0" draggable={false} />

      {/* Toolbar toggle button — hidden when expanded */}
      {!toolbarExpanded && (
        <button
          type="button"
          data-tauri-drag-region={false}
          onMouseDown={stopTitleBarDrag}
          onPointerDown={stopTitleBarDrag}
          onClick={() => setToolbarExpanded(true)}
          className="inline-flex h-8 w-8 items-center justify-center text-foreground/70 outline-none transition-colors hover:bg-surface-elevated/50 hover:text-foreground focus:outline-none focus-visible:outline-none"
          title={t('toolbar.expand')}>
          <AlignJustifyIcon className="size-4" />
        </button>
      )}

      {/* Expanded toolbar: "AI Proxy" + "Tools" menus with progressive gradient background */}
      <div
        ref={toolbarRef}
        data-tauri-drag-region={false}
        className={cn(
          'flex h-full items-center transition-all duration-200 ease-out',
          toolbarExpanded
            ? 'opacity-100 pointer-events-auto'
            : 'opacity-0 pointer-events-none absolute'
        )}
        style={toolbarExpanded ? {
          background: 'linear-gradient(to right, var(--surface-elevated) 0%, color-mix(in oklch, var(--surface-elevated) 60%, transparent) 40%, color-mix(in oklch, var(--surface-elevated) 20%, transparent) 70%, transparent 100%)',
        } : undefined}>
        <TitleBarMenu label="AI Proxy">
          {close => (
            <>
              <MenuItem
                onClick={() => {
                  close()
                  onOpenSettings()
                }}>
                <SettingsIcon className="size-4" />
                {t('menu.settings')}
              </MenuItem>
              <MenuItem
                onClick={() => {
                  close()
                  onOpenAbout()
                }}>
                <InfoIcon className="size-4" />
                {t('menu.about')}
              </MenuItem>
              <div className="my-1 h-px bg-surface-elevated" />
              <MenuItem
                variant="destructive"
                onClick={() => {
                  close()
                  void handleQuit()
                }}>
                <LogOutIcon className="size-4" />
                {t('menu.quit')}
              </MenuItem>
            </>
          )}
        </TitleBarMenu>
        <TitleBarMenu label={t('menu.tools')}>
          {close => (
            <>
              <MenuItem
                onClick={() => {
                  close()
                  onOpenSslConfig()
                }}>
                <ShieldCheckIcon className="size-4" />
                {t('menu.sslConfig')}
              </MenuItem>
              <MenuItem
                onClick={() => {
                  close()
                  onOpenScriptConfig()
                }}>
                <CodeIcon className="size-4" />
                {t('menu.scriptConfig')}
              </MenuItem>
            </>
          )}
        </TitleBarMenu>
      </div>

      {/* View tabs */}
      <TabBar activeView={activeView} mountedViews={mountedViews} onViewChange={onViewChange} onCloseTab={onCloseTab} />

      {/* Spacer: pushes right-side buttons to the far right */}
      <div className="min-w-0 flex-1" data-tauri-drag-region />

      {/* Far-right group: Delete, Edit, Start/Stop — order from left to right */}
      {!toolbarExpanded && (
        <div className="flex items-center gap-1" data-tauri-drag-region={false}>
          <button
            type="button"
            data-tauri-drag-region={false}
            onMouseDown={stopTitleBarDrag}
            onPointerDown={stopTitleBarDrag}
            onClick={onClearTraffic}
            className="inline-flex items-center rounded-md px-2 py-1 text-[11px] font-medium bg-surface-elevated text-muted-foreground border border-border hover:bg-muted transition-colors"
            title={t('traffic.clear')}>
            <Trash2Icon className="size-3" />
          </button>
          <button
            type="button"
            data-tauri-drag-region={false}
            onMouseDown={stopTitleBarDrag}
            onPointerDown={stopTitleBarDrag}
            onClick={onOpenSendRequest}
            className="inline-flex items-center rounded-md px-2 py-1 text-[11px] font-medium bg-surface-elevated text-muted-foreground border border-border hover:bg-muted transition-colors"
            title={t('sendRequest.title')}>
            <SquarePenIcon className="size-3" />
          </button>
          {running ? (
            <button
              type="button"
              data-tauri-drag-region={false}
              onMouseDown={stopTitleBarDrag}
              onPointerDown={stopTitleBarDrag}
              onClick={onStopProxy}
              className="inline-flex items-center rounded-md px-2 py-1 text-[11px] font-medium bg-destructive/10 text-destructive border border-destructive/20 hover:bg-destructive/20 transition-colors"
              title={t('app.stop')}>
              <SquareIcon className="size-3" />
            </button>
          ) : (
            <button
              type="button"
              data-tauri-drag-region={false}
              onMouseDown={stopTitleBarDrag}
              onPointerDown={stopTitleBarDrag}
              onClick={onStartProxy}
              className="inline-flex items-center rounded-md px-2 py-1 text-[11px] font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 hover:bg-emerald-500/20 transition-colors"
              title={t('app.start')}>
              <PlayIcon className="size-3" />
            </button>
          )}
        </div>
      )}

      <div className="flex h-full items-center" data-tauri-drag-region={false}>
        <WindowButton label="Minimize" onClick={() => void appWindow?.minimize()}>
          <MinusIcon className="size-3.5" />
        </WindowButton>
        <WindowButton label="Maximize" onClick={() => void appWindow?.toggleMaximize()}>
          <SquareIcon className="size-3" />
        </WindowButton>
        <WindowButton
          label="Close"
          onClick={() => void appWindow?.close()}
          className="hover:bg-destructive hover:text-destructive-foreground">
          <XIcon className="size-3.5" />
        </WindowButton>
      </div>
    </div>
  )
}
