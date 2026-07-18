import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useCopyToClipboard } from '@/hooks/useCopyToClipboard'
import { useTheme } from '@/hooks/useTheme'
import { useShiki } from '@/hooks/useShiki'
import { CopyIcon, CheckIcon, ChevronDown, ChevronRight } from 'lucide-react'
import { Tooltip, TooltipTrigger, TooltipContent } from '@/components/ui/tooltip'
import { Table, TableHeader, TableBody, TableHead, TableRow, TableCell } from '@/components/ui/table'
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from '@/components/ui/collapsible'

interface FormField {
  name: string
  value: string
  filename?: string
  contentType?: string
  headers: Record<string, string>
}

interface FormDataResult {
  type: 'urlencoded' | 'multipart'
  parts: FormField[]
}

function extractBoundary(contentType: string): string | null {
  const match = contentType.match(/boundary=([^;\s]+)/i)
  return match ? match[1] : null
}

function parseContentDisposition(header: string): { name: string; filename?: string } {
  const nameMatch = header.match(/name="([^"]*)"/i)
  const filenameMatch = header.match(/filename="([^"]*)"/i)
  return { name: nameMatch ? nameMatch[1] : '', filename: filenameMatch ? filenameMatch[1] : undefined }
}

function parseMultipartBody(body: string, boundary: string): FormField[] {
  const parts: FormField[] = []
  const delimiter = '--' + boundary
  const chunks = body.split(delimiter)
  for (const chunk of chunks) {
    const trimmed = chunk.replace(/^[\r\n]+|[\r\n]+$/g, '')
    if (!trimmed || trimmed === '--') continue
    const sepIdx = trimmed.search(/\r?\n\r?\n/)
    if (sepIdx === -1) continue
    const headerSection = trimmed.slice(0, sepIdx)
    const bodySection = trimmed.slice(sepIdx + 1).replace(/^[\r\n]+/, '')
    const cleanBody = bodySection.replace(/[\r\n]*--\s*$/, '')
    const headers: Record<string, string> = {}
    let dispositionName = ''
    let filename: string | undefined
    let contentType: string | undefined
    for (const line of headerSection.split('\n')) {
      const l = line.trim()
      if (!l) continue
      const colonIdx = l.indexOf(':')
      if (colonIdx === -1) continue
      const key = l.slice(0, colonIdx).trim()
      const value = l.slice(colonIdx + 1).trim()
      headers[key] = value
      if (key.toLowerCase() === 'content-disposition') {
        const parsed = parseContentDisposition(value)
        dispositionName = parsed.name
        filename = parsed.filename
      }
      if (key.toLowerCase() === 'content-type') contentType = value
    }
    parts.push({ name: dispositionName, value: cleanBody, filename, contentType, headers })
  }
  return parts
}

function parseUrlEncodedBody(body: string): FormField[] {
  const parts: FormField[] = []
  const pairs = body.split('&')
  for (const pair of pairs) {
    if (!pair.trim()) continue
    const eqIdx = pair.indexOf('=')
    if (eqIdx === -1) {
      parts.push({ name: decodeURIComponent(pair.trim()), value: '', headers: {} })
    } else {
      parts.push({ name: decodeURIComponent(pair.slice(0, eqIdx).trim()), value: decodeURIComponent(pair.slice(eqIdx + 1).trim()), headers: {} })
    }
  }
  return parts
}

export function parseFormData(body: string, contentTypeHeader: string | undefined): FormDataResult | null {
  if (!body || !contentTypeHeader) return null
  const ct = contentTypeHeader.toLowerCase()
  if (ct.includes('application/x-www-form-urlencoded')) return { type: 'urlencoded', parts: parseUrlEncodedBody(body) }
  if (ct.includes('multipart/form-data')) {
    const boundary = extractBoundary(contentTypeHeader)
    if (!boundary) return null
    return { type: 'multipart', parts: parseMultipartBody(body, boundary) }
  }
  return null
}

export function hasFormDataContent(headers: Record<string, string> | undefined | null): boolean {
  if (!headers) return false
  const ct = Object.entries(headers).find(([k]) => k.toLowerCase() === 'content-type')?.[1]
  if (!ct) return false
  const lc = ct.toLowerCase()
  return lc.includes('application/x-www-form-urlencoded') || lc.includes('multipart/form-data')
}

export default function FormDataView({ body, contentType }: { body: string; contentType: string | undefined }) {
  const { t } = useTranslation()
  const result = useMemo(() => parseFormData(body, contentType), [body, contentType])
  if (!result) return <div className='flex items-center justify-center py-10 text-sm text-muted-foreground'>{t('detail.noFormData')}</div>
  if (result.type === 'urlencoded') return <UrlEncodedView parts={result.parts} />
  return <MultipartView parts={result.parts} />
}

function UrlEncodedView({ parts }: { parts: FormField[] }) {
  const { t } = useTranslation()
  return (
    <Table className='table-fixed text-xs'>
      <colgroup><col style={{ width: '30%' }} /><col style={{ width: '70%' }} /></colgroup>
      <TableHeader>
        <TableRow className='border-b border-surface-elevated bg-surface-elevated/30 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground'>
          <TableHead className='py-1.5 pl-3 pr-2 h-auto text-left font-semibold text-[10px]'>Key</TableHead>
          <TableHead className='py-1.5 pr-3 h-auto text-left font-semibold text-[10px]'>Value</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {parts.length === 0 ? (
          <TableRow><TableCell colSpan={2} className='py-4 text-center text-muted-foreground'>{t('detail.noFormData')}</TableCell></TableRow>
        ) : (
          parts.map((part) => (
            <TableRow key={part.name} className='border-b border-surface-elevated/30 hover:bg-surface-elevated/20 transition-colors'>
              <TableCell className='py-1.5 pl-3 pr-2 align-top font-medium text-foreground/90 whitespace-nowrap overflow-hidden text-ellipsis'>{part.name}</TableCell>
              <TableCell className='py-1.5 pr-3 align-top text-foreground/70 break-all'>{part.value}</TableCell>
            </TableRow>
          ))
        )}
      </TableBody>
    </Table>
  )
}

function MultipartView({ parts }: { parts: FormField[] }) {
  return (
    <div className='flex flex-col gap-2 p-2'>
      {parts.length === 0 ? <div className='flex items-center justify-center py-6 text-sm text-muted-foreground'>(empty)</div> : parts.map((part, idx) => <MultipartPartCard key={part.name + '-' + idx} part={part} />)}
    </div>
  )
}

function MultipartPartCard({ part }: { part: FormField }) {
  const [expanded, setExpanded] = useState(true)
  const { copied, copy } = useCopyToClipboard()
  const { resolvedTheme } = useTheme()
  const shikiTheme = resolvedTheme === 'dark' ? 'github-dark' : 'github-light'
  const lang = part.contentType?.toLowerCase().includes('json') ? 'json' : part.contentType?.toLowerCase().includes('html') ? 'html' : part.contentType?.toLowerCase().includes('xml') ? 'xml' : 'plaintext'
  const highlightedBody = useShiki(part.value, lang, shikiTheme)
  const headerCount = Object.keys(part.headers).length
  const fileInfo = part.filename ? part.filename + (part.contentType ? ' (' + part.contentType + ')' : '') : null
  return (
    <Collapsible open={expanded} onOpenChange={setExpanded} className='rounded border border-surface-elevated overflow-hidden'>
      <div className='flex items-center gap-2 px-2.5 py-1.5 bg-surface-elevated/20 border-b border-surface-elevated hover:bg-surface-elevated/30 transition-colors select-none'>
        <CollapsibleTrigger className="shrink-0 p-0 rounded">
          {expanded ? <ChevronDown className='size-3 shrink-0 text-muted-foreground/60' /> : <ChevronRight className='size-3 shrink-0 text-muted-foreground/60' />}
        </CollapsibleTrigger>
        <span className='text-xs font-medium text-foreground/90 truncate min-w-0'>{part.name}</span>
        {fileInfo ? <span className='text-[10px] text-muted-foreground/70 truncate min-w-0 shrink-0'>{fileInfo}</span> : <span className='text-[10px] text-muted-foreground/50 shrink-0'>{part.value.length}B</span>}
      </div>
      <CollapsibleContent>
        <div className='text-xs'>
          {headerCount > 0 && (
            <div className='border-b border-surface-elevated/30 bg-surface-elevated/10 px-2.5 py-1'>
              {Object.entries(part.headers).map(([key, value]) => (
                <div key={key} className='flex gap-2 text-[10px] text-muted-foreground'>
                  <span className='font-medium shrink-0'>{key}:</span><span className='break-all'>{value}</span>
                </div>
              ))}
            </div>
          )}
          <div className='relative group/mini'>
            <div className='absolute top-1 right-1 z-10 opacity-0 group-hover/mini:opacity-100 transition-all'>
              <Tooltip>
                <TooltipTrigger className="inline-flex">
                  <button onClick={() => copy(part.value)} className='rounded p-1 text-muted-foreground hover:text-foreground hover:bg-surface-elevated/50 transition-colors'>{copied ? <CheckIcon className='size-3 text-primary' /> : <CopyIcon className='size-3' />}</button>
                </TooltipTrigger>
                <TooltipContent side="left" className="bg-popover text-popover-foreground text-[11px]">
                  {copied ? 'Copied' : 'Copy'}
                </TooltipContent>
              </Tooltip>
            </div>
            {highlightedBody ? <div className='shiki-root whitespace-pre-wrap break-all overflow-x-auto px-2.5 py-1.5' dangerouslySetInnerHTML={{ __html: highlightedBody }} /> : <pre className='whitespace-pre-wrap break-all px-2.5 py-1.5 text-foreground/80 font-mono'>{part.value}</pre>}
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}
