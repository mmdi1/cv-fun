export type SettingsSectionId = 'hotkeys' | 'history' | 'parsing'

export type SettingsSection = {
  id: SettingsSectionId
  title: string
  description: string
  items: string[]
}

export type ParserOption = {
  id: string
  label: string
}

export const SETTINGS_SECTIONS: SettingsSection[] = [
  {
    id: 'hotkeys',
    title: '快捷键',
    description: '配置打开剪贴板历史、监听复制和粘贴历史项的快捷键。',
    items: ['打开历史面板', '监听复制快捷键', '粘贴最近历史'],
  },
  {
    id: 'history',
    title: '历史记录',
    description: '控制历史保留数量、清理策略和本地存储位置。',
    items: ['最大历史条数', '清理未置顶内容', '忽略下一次复制'],
  },
  {
    id: 'parsing',
    title: '内容解析',
    description: '管理时间戳、JSON、URL、英文单词等解析规则。',
    items: ['时间戳格式化', 'JSON 格式化', '英文单词翻译'],
  },
]

export const PARSER_OPTIONS: ParserOption[] = [
  { id: 'timestamp', label: '时间戳' },
  { id: 'datetime', label: '日期时间' },
  { id: 'wordTranslation', label: '英文单词' },
  { id: 'json', label: 'JSON' },
  { id: 'url', label: 'URL' },
  { id: 'base64', label: 'Base64' },
  { id: 'color', label: '颜色值' },
  { id: 'uuid', label: 'UUID' },
]
