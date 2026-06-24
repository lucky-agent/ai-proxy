import { PlusIcon, MessageSquareIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'
import { Separator } from '@/components/ui/separator'
import { ScrollArea } from '@/components/ui/scroll-area'

export function AiSidebar() {
  const { t } = useLocale()

  return (
    <div className="flex h-full flex-col bg-surface-base/30">
      {/* 标题栏 */}
      <div className="flex items-center px-3 py-2">
        <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
          {t('aiSidebar.title')}
        </span>
      </div>

      <Separator />

      {/* 会话列表 */}
      <ScrollArea className="flex-1 min-h-0">
        <div className="flex flex-col items-center justify-center gap-2 py-8 px-4 text-muted-foreground">
          <MessageSquareIcon className="size-8 text-muted-foreground/25" />
          <p className="text-xs">{t('aiSidebar.empty')}</p>
        </div>
      </ScrollArea>

      <Separator />

      {/* 操作按钮 */}
      <div className="flex items-center gap-1 px-3 py-2">
        <button
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <PlusIcon className="size-3.5" />
          {t('aiSidebar.newChat')}
        </button>
      </div>
    </div>
  )
}
