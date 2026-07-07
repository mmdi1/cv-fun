import test from 'node:test'
import assert from 'node:assert/strict'
import { analyzeClipboardText, formatHistoryTime } from './analyzer.ts'

test('formats second-level unix timestamps', () => {
  const cards = analyzeClipboardText('1719820800')

  assert.equal(cards[0]?.type, 'timestamp')
  assert.match(cards[0]?.value ?? '', /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/)
  assert.equal(cards[0]?.hint, '按秒级时间戳解析')
})

test('formats valid JSON and reports its shape', () => {
  const cards = analyzeClipboardText('{"name":"ntools","items":[1,2]}')
  const jsonCard = cards.find((card) => card.type === 'json')

  assert.ok(jsonCard)
  assert.equal(jsonCard.title, 'JSON')
  assert.match(jsonCard.value, /"name": "ntools"/)
  assert.equal(jsonCard.hint, '对象格式有效')
})

test('detects URL and English words without network calls', () => {
  const urlCard = analyzeClipboardText('https://example.com/a?b=1').find((card) => card.type === 'url')
  const wordCard = analyzeClipboardText('clipboard').find((card) => card.type === 'word')

  assert.equal(urlCard?.value, 'https://example.com/a')
  assert.equal(urlCard?.hint, 'example.com')
  assert.equal(wordCard?.hint, '已识别为单词，下一步接入翻译源')
})

test('shows only time for history records copied today', () => {
  const now = new Date('2026-07-06T12:30:00')

  assert.equal(formatHistoryTime('2026-07-06T08:09:10', now), '08:09:10')
})

test('shows date and time for older history records', () => {
  const now = new Date('2026-07-06T12:30:00')

  assert.equal(formatHistoryTime('2026-07-05T08:09:10', now), '07-05 08:09')
})
