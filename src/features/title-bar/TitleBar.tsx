import { useEffect, useRef, useState, type MouseEvent, type ReactNode } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  InfoIcon,
  LogOutIcon,
  MinusIcon,
  SettingsIcon,
  SquareIcon,
  XIcon,
} from 'lucide-react'
import appIcon from '@/assets/app-icon.png'
import { useLocale } from '@/hooks/useLocale'
import { cn } from '@/lib/utils'

type TitleBarProps = {
  onOpenSettings: () => void
  onOpenAbout: () => void
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

  useEffect(() => {
    if (!open) return

    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false)
      }
    }

    document.addEventListener('pointerdown', onPointerDown, true)
    return () => document.removeEventListener('pointerdown', onPointerDown, true)
  }, [open])

  function close() {
    setOpen(false)
  }

  return (
    <div ref={rootRef} className="relative h-full" data-tauri-drag-region={false}>
      <button
        type="button"
        data-tauri-drag-region={false}
        aria-expanded={open}
        aria-haspopup="menu"
        onMouseDown={stopTitleBarDrag}
        onPointerDown={stopTitleBarDrag}
        onClick={() => setOpen(value => !value)}
        className={cn(
          'inline-flex h-8 items-center rounded-none border-0 px-2.5 text-xs text-foreground/80 shadow-none outline-none transition-colors hover:bg-muted hover:text-foreground focus:outline-none focus-visible:outline-none',
          open && 'bg-muted text-foreground',
        )}>
        {label}
      </button>
      {open ? (
        <div
          role="menu"
          data-tauri-drag-region={false}
          className="absolute top-full left-0 z-[200] min-w-32 rounded-lg bg-popover p-1 text-popover-foreground shadow-md ring-1 ring-foreground/10">
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
        variant === 'destructive' && 'text-destructive hover:bg-destructive/10 hover:text-destructive',
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
        'inline-flex h-8 w-11 items-center justify-center text-foreground/70 outline-none transition-colors hover:bg-muted hover:text-foreground focus:outline-none focus-visible:outline-none',
        className,
      )}>
      {children}
    </button>
  )
}

export function TitleBar({ onOpenSettings, onOpenAbout }: TitleBarProps) {
  const { t } = useLocale()
  const appWindow = getCurrentWindow()

  async function handleQuit() {
    await appWindow.close()
  }

  function handleTitleBarMouseDown(event: MouseEvent<HTMLDivElement>) {
    const target = event.target as HTMLElement
    if (target.closest('[data-tauri-drag-region="false"]')) return
    if (event.button !== 0) return

    if (event.detail === 2) {
      void appWindow.toggleMaximize()
      return
    }

    void appWindow.startDragging()
  }

  return (
    <div
      className="titlebar flex h-8 shrink-0 items-center border-b border-border bg-background select-none"
      data-tauri-drag-region
      onMouseDown={handleTitleBarMouseDown}>
      <img src={appIcon} alt="" className="ml-2 size-4 shrink-0" draggable={false} />
      <nav className="flex h-full items-center" data-tauri-drag-region={false}>
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
              <div className="my-1 h-px bg-border" />
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
      </nav>

      <div className="min-w-0 flex-1" />

      <div className="flex h-full items-center" data-tauri-drag-region={false}>
        <WindowButton label="Minimize" onClick={() => void appWindow.minimize()}>
          <MinusIcon className="size-3.5" />
        </WindowButton>
        <WindowButton label="Maximize" onClick={() => void appWindow.toggleMaximize()}>
          <SquareIcon className="size-3" />
        </WindowButton>
        <WindowButton
          label="Close"
          onClick={() => void appWindow.close()}
          className="hover:bg-destructive hover:text-destructive-foreground">
          <XIcon className="size-3.5" />
        </WindowButton>
      </div>
    </div>
  )
}
