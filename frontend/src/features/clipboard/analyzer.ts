export type AnalysisCard = {
  type: 'timestamp' | 'json' | 'url' | 'word' | 'stats'
  title: string
  value: string
  hint: string
}

export function formatDateTime(value: unknown): string {
  if (!value) {
    return '-'
  }

  const date = new Date(String(value))
  if (Number.isNaN(date.getTime())) {
    return String(value)
  }

  const pad = (num: number) => String(num).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

export function formatHistoryTime(value: unknown, now = new Date()): string {
  if (!value) {
    return '-'
  }

  const date = new Date(String(value))
  if (Number.isNaN(date.getTime())) {
    return String(value)
  }

  const pad = (num: number) => String(num).padStart(2, '0')
  const sameDay =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate()

  if (sameDay) {
    return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  }

  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

export function analyzeClipboardText(text: string): AnalysisCard[] {
  const trimmed = text.trim()
  const cards: AnalysisCard[] = []

  if (!trimmed) {
    return cards
  }

  const timestampMatch = /^-?\d{10}(\d{3})?$/.exec(trimmed)
  if (timestampMatch) {
    const raw = Number(trimmed)
    const milliseconds = trimmed.length === 13 ? raw : raw * 1000
    const date = new Date(milliseconds)
    if (!Number.isNaN(date.getTime())) {
      cards.push({
        type: 'timestamp',
        title: '时间戳',
        value: formatDateTime(date.toISOString()),
        hint: trimmed.length === 13 ? '按毫秒时间戳解析' : '按秒级时间戳解析',
      })
    }
  }

  if (/^[{[]/.test(trimmed)) {
    try {
      const parsed = JSON.parse(trimmed)
      cards.push({
        type: 'json',
        title: 'JSON',
        value: JSON.stringify(parsed, null, 2),
        hint: Array.isArray(parsed) ? `数组，${parsed.length} 项` : '对象格式有效',
      })
    } catch {
      cards.push({
        type: 'json',
        title: 'JSON',
        value: '解析失败',
        hint: '看起来像 JSON，但语法暂未通过',
      })
    }
  }

  try {
    const url = new URL(trimmed)
    cards.push({
      type: 'url',
      title: 'URL',
      value: `${url.protocol}//${url.host}${url.pathname}`,
      hint: url.hostname,
    })
  } catch {
    // Not a URL.
  }

  if (/^[A-Za-z][A-Za-z'-]{1,}$/.test(trimmed)) {
    cards.push({
      type: 'word',
      title: '英文单词',
      value: trimmed,
      hint: '已识别为单词，下一步接入翻译源',
    })
  }

  cards.push({
    type: 'stats',
    title: '文本统计',
    value: `${Array.from(text).length} 字符`,
    hint: `${text.split(/\r\n|\r|\n/).length} 行`,
  })

  return cards
}
