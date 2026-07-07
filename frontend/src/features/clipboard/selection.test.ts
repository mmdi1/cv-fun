import test from 'node:test'
import assert from 'node:assert/strict'
import { resolveSelectedHistoryId } from './selection.ts'

test('selects the latest record after a new clipboard item arrives', () => {
  const result = resolveSelectedHistoryId({
    records: [{ id: 'new' }, { id: 'old' }],
    currentSelectedId: 'old',
    lastTopId: 'old',
  })

  assert.equal(result.selectedId, 'new')
  assert.equal(result.lastTopId, 'new')
})

test('keeps the user selected record when the latest record did not change', () => {
  const result = resolveSelectedHistoryId({
    records: [{ id: 'latest' }, { id: 'older' }],
    currentSelectedId: 'older',
    lastTopId: 'latest',
  })

  assert.equal(result.selectedId, 'older')
  assert.equal(result.lastTopId, 'latest')
})

test('falls back to latest record when the selected record disappears', () => {
  const result = resolveSelectedHistoryId({
    records: [{ id: 'latest' }],
    currentSelectedId: 'deleted',
    lastTopId: 'latest',
  })

  assert.equal(result.selectedId, 'latest')
  assert.equal(result.lastTopId, 'latest')
})
