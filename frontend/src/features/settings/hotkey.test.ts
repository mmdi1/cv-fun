import test from 'node:test'
import assert from 'node:assert/strict'
import { formatKeyboardShortcut } from './hotkey.ts'

test('formats mac style keyboard shortcuts in a stable order', () => {
  assert.equal(
    formatKeyboardShortcut({ key: 'c', metaKey: true, shiftKey: true }),
    'Command+Shift+C',
  )
})

test('formats windows style copy shortcut', () => {
  assert.equal(formatKeyboardShortcut({ key: 'c', ctrlKey: true }), 'Control+C')
})

test('formats option based shortcuts', () => {
  assert.equal(
    formatKeyboardShortcut({ key: 'c', metaKey: true, altKey: true }),
    'Command+Option+C',
  )
})

test('waits for a non-modifier key before producing a shortcut', () => {
  assert.equal(formatKeyboardShortcut({ key: 'Meta', metaKey: true }), '')
  assert.equal(formatKeyboardShortcut({ key: 'Control', ctrlKey: true }), '')
  assert.equal(formatKeyboardShortcut({ key: 'Shift', shiftKey: true }), '')
})

test('normalizes special keys', () => {
  assert.equal(formatKeyboardShortcut({ key: ' ', metaKey: true }), 'Command+Space')
  assert.equal(formatKeyboardShortcut({ key: 'ArrowUp', ctrlKey: true }), 'Control+ArrowUp')
})
