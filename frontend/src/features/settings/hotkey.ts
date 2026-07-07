export type KeyboardShortcutEvent = {
  key: string
  metaKey?: boolean
  ctrlKey?: boolean
  altKey?: boolean
  shiftKey?: boolean
}

const modifierKeys = new Set([
  'Alt',
  'Control',
  'Meta',
  'Shift',
  'Option',
  'Command',
])

const specialKeyLabels: Record<string, string> = {
  ' ': 'Space',
  Escape: 'Escape',
  Esc: 'Escape',
  Enter: 'Enter',
  Return: 'Enter',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Tab: 'Tab',
  ArrowUp: 'ArrowUp',
  ArrowDown: 'ArrowDown',
  ArrowLeft: 'ArrowLeft',
  ArrowRight: 'ArrowRight',
}

export function formatKeyboardShortcut(event: KeyboardShortcutEvent): string {
  if (modifierKeys.has(event.key)) {
    return ''
  }

  const keys: string[] = []
  if (event.metaKey) {
    keys.push('Command')
  }
  if (event.ctrlKey) {
    keys.push('Control')
  }
  if (event.altKey) {
    keys.push('Option')
  }
  if (event.shiftKey) {
    keys.push('Shift')
  }

  keys.push(normalizeShortcutKey(event.key))
  return keys.join('+')
}

function normalizeShortcutKey(key: string): string {
  if (specialKeyLabels[key]) {
    return specialKeyLabels[key]
  }
  if (key.length === 1) {
    return key.toUpperCase()
  }
  return key
}
