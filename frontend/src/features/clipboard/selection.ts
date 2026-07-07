export type SelectableHistoryRecord = {
  id: string
}

export type ResolveSelectedHistoryInput<T extends SelectableHistoryRecord> = {
  records: T[]
  currentSelectedId: string
  lastTopId: string
}

export type ResolveSelectedHistoryResult = {
  selectedId: string
  lastTopId: string
}

export function resolveSelectedHistoryId<T extends SelectableHistoryRecord>(
  input: ResolveSelectedHistoryInput<T>,
): ResolveSelectedHistoryResult {
  const latest = input.records[0]
  if (!latest) {
    return { selectedId: '', lastTopId: '' }
  }

  const latestId = latest.id
  const selectedExists = input.records.some((record) => record.id === input.currentSelectedId)
  const latestChanged = input.lastTopId !== '' && input.lastTopId !== latestId

  if (input.currentSelectedId === '' || !selectedExists || latestChanged) {
    return { selectedId: latestId, lastTopId: latestId }
  }

  return { selectedId: input.currentSelectedId, lastTopId: latestId }
}
