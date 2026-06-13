import { SparklesIcon } from 'lucide-react'
import { useLocale } from '@/hooks/useLocale'

export function AiView() {
  const { t } = useLocale()

  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 bg-surface-deep text-muted-foreground">
      <SparklesIcon className="size-12 text-muted-foreground/30" />
      <p className="text-sm font-medium">{t('view.aiComingSoon')}</p>
    </div>
  )
}
