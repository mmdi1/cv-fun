<script setup lang="ts">
import { ref } from 'vue'
import { formatKeyboardShortcut } from './hotkey'

defineProps<{
  label: string
  modelValue: string
  hint?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const recording = ref(false)

function startRecording() {
  recording.value = true
}

function stopRecording() {
  recording.value = false
}

function recordShortcut(event: KeyboardEvent) {
  event.preventDefault()
  event.stopPropagation()

  if (event.key === 'Escape' && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
    stopRecording()
    return
  }

  const shortcut = formatKeyboardShortcut(event)
  if (!shortcut) {
    return
  }

  emit('update:modelValue', shortcut)
  stopRecording()
}
</script>

<template>
  <label class="hotkey-field">
    <span>{{ label }}</span>
    <input
      :value="recording ? '请按下快捷键...' : modelValue"
      aria-keyshortcuts="Enter Space"
      readonly
      type="text"
      @blur="stopRecording"
      @click="startRecording"
      @focus="startRecording"
      @keydown="recordShortcut"
    />
    <small v-if="hint">{{ hint }}</small>
  </label>
</template>

<style scoped>
.hotkey-field {
  display: grid;
  gap: 6px;
  color: #98a2b3;
  font-size: 12px;
}

.hotkey-field input {
  width: 100%;
  border: 1px solid #323847;
  border-radius: 8px;
  outline: none;
  color: #f6f7fb;
  background: #10141d;
  padding: 8px 9px;
  font-size: 13px;
  cursor: text;
}

.hotkey-field input:focus {
  border-color: #64748b;
  box-shadow: 0 0 0 2px rgba(100, 116, 139, 0.22);
}

.hotkey-field small {
  color: #748096;
  font-size: 11px;
  line-height: 1.4;
}
</style>
