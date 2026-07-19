import { useEffect, useRef } from 'react'
import { EditorState } from '@codemirror/state'
import {
  EditorView,
  keymap,
  lineNumbers,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  drawSelection,
  dropCursor,
  rectangularSelection,
  crosshairCursor,
} from '@codemirror/view'
import { highlightSelectionMatches } from '@codemirror/search'
import {
  syntaxHighlighting,
  indentOnInput,
  bracketMatching,
  foldGutter,
  foldKeymap,
} from '@codemirror/language'
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
} from '@codemirror/commands'
import {
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
} from '@codemirror/autocomplete'
import { searchKeymap } from '@codemirror/search'
import { lintKeymap } from '@codemirror/lint'
import { classHighlighter } from '@lezer/highlight'
import { json as jsonLang } from '@codemirror/lang-json'
import { xml as xmlLang } from '@codemirror/lang-xml'
import { javascript as jsLang } from '@codemirror/lang-javascript'
import type { BodyType } from '@/types/collection'

interface CodeEditorProps {
  value: string
  language: BodyType | 'javascript'
  onChange: (value: string) => void
}

export default function CodeEditor({ value, language, onChange }: CodeEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange
  const langRef = useRef(language)
  const extsRef = useRef<any[] | null>(null)

function detectLanguage(content: string): 'json' | 'xml' | null {
    const trimmed = content.trim()
    if (trimmed.startsWith('{') || trimmed.startsWith('[')) return 'json'
    if (trimmed.startsWith('<')) return 'xml'
    return null
  }

  // Build extensions set once per language
  if (extsRef.current === null || langRef.current !== language) {
    langRef.current = language
    const effectiveLang = language === 'auto'
      ? detectLanguage(value) ?? 'text'
      : language
    const lang = effectiveLang === 'json' ? jsonLang() : effectiveLang === 'xml' ? xmlLang() : effectiveLang === 'javascript' ? jsLang() : null
    const exts: any[] = [
      lineNumbers(),
      highlightActiveLineGutter(),
      highlightSpecialChars(),
      history(),
      foldGutter(),
      drawSelection(),
      dropCursor(),
      EditorState.allowMultipleSelections.of(true),
      indentOnInput(),
      syntaxHighlighting(classHighlighter),
      bracketMatching(),
      closeBrackets(),
      rectangularSelection(),
      crosshairCursor(),
      highlightActiveLine(),
      highlightSelectionMatches(),
      keymap.of([
        ...closeBracketsKeymap,
        ...defaultKeymap,
        ...searchKeymap,
        ...historyKeymap,
        ...foldKeymap,
        ...completionKeymap,
        ...lintKeymap,
        indentWithTab,
      ]),
      EditorView.theme(
        {
          '&': { height: '100%', backgroundColor: 'transparent' },
          '.cm-gutters': { backgroundColor: 'transparent', color: 'var(--color-muted-foreground)', border: 'none' },
          '.cm-activeLineGutter': { backgroundColor: 'var(--color-surface-elevated)' },
          '.cm-activeLine': { backgroundColor: 'var(--color-surface-elevated)' },
          '.cm-cursor': { borderLeftColor: '#528bff' },
          '.cm-matchingBracket': { backgroundColor: 'var(--color-surface-elevated)', outline: '1px solid var(--color-muted-foreground)' },
          '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': {
            backgroundColor: 'var(--color-surface-elevated)',
          },
          '.cm-scroller': {
            fontFamily: 'var(--font-sans, "Geist Variable", "Menlo", monospace)',
            fontSize: 'var(--text-prose-lg, 0.875rem)', // 14px @scale=1，随「内容字号」设置缩放
          },
        },
        { dark: true },
      ),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          onChangeRef.current(update.state.doc.toString())
        }
      }),
    ]
    if (lang) exts.push(lang)
    extsRef.current = exts
  }

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    // Destroy previous view (language switch)
    viewRef.current?.destroy()

    const state = EditorState.create({
      doc: value,
      extensions: extsRef.current!,
    })
    const view = new EditorView({ state, parent: container })
    viewRef.current = view

    return () => {
      view.destroy()
      viewRef.current = null
    }
  }, [language])

  // Sync external value into the editor
  useEffect(() => {
    const view = viewRef.current
    if (!view) return
    const current = view.state.doc.toString()
    if (value !== current) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: value },
      })
    }
  }, [value])

  return <div ref={containerRef} className="size-full overflow-auto" />
}
