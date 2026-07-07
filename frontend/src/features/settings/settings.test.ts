import test from 'node:test'
import assert from 'node:assert/strict'
import { PARSER_OPTIONS, SETTINGS_SECTIONS } from './config.ts'

test('defines the first settings sections for clipboard workflow', () => {
  assert.deepEqual(
    SETTINGS_SECTIONS.map((section) => section.id),
    ['hotkeys', 'history', 'parsing'],
  )
  assert.equal(SETTINGS_SECTIONS[0]?.title, '快捷键')
  assert.match(SETTINGS_SECTIONS[0]?.description ?? '', /打开剪贴板历史/)
  assert.deepEqual(SETTINGS_SECTIONS[0]?.items, ['打开历史面板', '监听复制快捷键', '粘贴最近历史'])
})

test('defines parser options that match the backend config keys', () => {
  assert.deepEqual(
    PARSER_OPTIONS.map((option) => option.id),
    ['timestamp', 'datetime', 'wordTranslation', 'json', 'url', 'base64', 'color', 'uuid'],
  )
})
