import { PlusIcon, Trash2Icon } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { Empty, EmptyTitle } from '@/components/ui/empty'
import type { KeyValuePair } from '@/types/collection'

interface KeyValueEditorProps {
  entries: KeyValuePair[]
  onChange: (entries: KeyValuePair[]) => void
  title: string
  addLabel: string
  emptyLabel: string
}

export function KeyValueEditor({ entries, onChange, title, addLabel, emptyLabel }: KeyValueEditorProps) {
  const handleChange = (i: number, field: 'key' | 'value', val: string) => {
    onChange(entries.map((e, idx) => idx === i ? { ...e, [field]: val } : e))
  }

  const handleRemove = (i: number) => {
    onChange(entries.filter((_, idx) => idx !== i))
  }

  const handleAdd = () => {
    onChange([...entries, { key: '', value: '' }])
  }

  return (
    <div className="p-4 space-y-1 min-h-0 flex flex-col">
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
          {title}
        </span>
        <button
          onClick={handleAdd}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <PlusIcon className="size-3" />
          {addLabel}
        </button>
      </div>
      {entries.length === 0 ? (
        <div className="py-8 text-center">
          <Empty><EmptyTitle>{emptyLabel}</EmptyTitle></Empty>
        </div>
      ) : (
        <div className="space-y-1">
          {entries.map((pair, i) => (
            <div key={i} className="flex gap-1 items-center">
              <Input
                value={pair.key}
                onChange={e => handleChange(i, 'key', e.target.value)}
                className="flex-1 h-auto py-1 text-[11px] font-mono"
                placeholder="Key"
              />
              <Input
                value={pair.value}
                onChange={e => handleChange(i, 'value', e.target.value)}
                className="flex-[2] h-auto py-1 text-[11px] font-mono"
                placeholder="Value"
              />
              <button
                onClick={() => handleRemove(i)}
                className="shrink-0 rounded p-1 text-muted-foreground hover:text-destructive transition-colors"
              >
                <Trash2Icon className="size-3" />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
