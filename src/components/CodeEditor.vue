<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { CODE_LANG_LABEL, detectCodeLang, type CodeLang } from "../utils/langDetect";
import { highlightCode } from "../utils/highlight";

const props = defineProps<{
  modelValue: string;
  disabled?: boolean;
  placeholder?: string;
  /** Hint from applied suggestion id, e.g. sql:pretty */
  langHint?: string | null;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const ta = ref<HTMLTextAreaElement | null>(null);
const pre = ref<HTMLElement | null>(null);

const lang = computed<CodeLang>(() => detectCodeLang(props.modelValue, props.langHint));
const langLabel = computed(() => CODE_LANG_LABEL[lang.value]);
const highlighted = computed(() => highlightCode(props.modelValue, lang.value));

function onInput(e: Event) {
  const el = e.target as HTMLTextAreaElement;
  emit("update:modelValue", el.value);
  void syncScroll();
}

function syncScroll() {
  if (!ta.value || !pre.value) return;
  pre.value.scrollTop = ta.value.scrollTop;
  pre.value.scrollLeft = ta.value.scrollLeft;
}

watch(
  () => props.modelValue,
  async () => {
    await nextTick();
    syncScroll();
  },
);

defineExpose({ lang, langLabel });
</script>

<template>
  <div class="code-stack" :data-lang="lang">
    <pre
      ref="pre"
      class="code-hl hljs"
      aria-hidden="true"
    ><code class="hljs" v-html="highlighted + '\n'" /></pre>
    <textarea
      ref="ta"
      class="code-input"
      :value="modelValue"
      :disabled="disabled"
      :placeholder="placeholder"
      spellcheck="false"
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      @input="onInput"
      @scroll="syncScroll"
    />
  </div>
</template>

<style scoped>
.code-stack {
  position: relative;
  flex: 1 1 auto;
  min-height: 120px;
  height: 100%;
  overflow: hidden;
}

.code-hl,
.code-input {
  margin: 0;
  padding: 0;
  border: 0;
  width: 100%;
  height: 100%;
  min-height: 120px;
  box-sizing: border-box;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.84rem;
  line-height: 1.55;
  tab-size: 2;
}

.code-hl {
  position: absolute;
  inset: 0;
  overflow: auto;
  pointer-events: none;
  color: #e6edf3;
  background: transparent;
  /* hide scrollbar */
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.code-hl::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.code-hl code {
  display: block;
  font: inherit;
  background: transparent !important;
  padding: 0 !important;
  white-space: inherit;
  word-break: inherit;
}

.code-input {
  position: relative;
  z-index: 1;
  display: block;
  resize: none;
  outline: none;
  background: transparent;
  /* Text transparent so highlight shows through; caret stays visible */
  color: transparent;
  caret-color: #3ecfad;
  overflow: auto;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.code-input::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.code-input:disabled {
  opacity: 0.55;
  cursor: wait;
}

.code-input::placeholder {
  color: #7a8792;
  opacity: 0.75;
}

/* Selection still readable */
.code-input::selection {
  background: rgba(62, 207, 173, 0.28);
  color: transparent;
}
</style>

<!-- Global-ish tokens for hljs classes inside this component -->
<style>
/* Dark theme tokens matching FunCV */
.code-stack .hljs {
  background: transparent;
  color: #e6edf3;
}
.code-stack .hljs-keyword,
.code-stack .hljs-selector-tag,
.code-stack .hljs-literal,
.code-stack .hljs-section,
.code-stack .hljs-link {
  color: #c792ea;
}
.code-stack .hljs-string,
.code-stack .hljs-doctag,
.code-stack .hljs-addition {
  color: #c3e88d;
}
.code-stack .hljs-number,
.code-stack .hljs-bullet {
  color: #f78c6c;
}
.code-stack .hljs-built_in,
.code-stack .hljs-type,
.code-stack .hljs-class .hljs-title,
.code-stack .hljs-title.class_ {
  color: #ffcb6b;
}
.code-stack .hljs-title,
.code-stack .hljs-title.function_,
.code-stack .hljs-attr {
  color: #82aaff;
}
.code-stack .hljs-comment,
.code-stack .hljs-quote,
.code-stack .hljs-deletion,
.code-stack .hljs-meta {
  color: #697098;
  font-style: italic;
}
.code-stack .hljs-variable,
.code-stack .hljs-template-variable,
.code-stack .hljs-regexp,
.code-stack .hljs-selector-attr,
.code-stack .hljs-selector-pseudo,
.code-stack .hljs-symbol {
  color: #f07178;
}
.code-stack .hljs-name,
.code-stack .hljs-selector-id,
.code-stack .hljs-selector-class {
  color: #89ddff;
}
.code-stack .hljs-property {
  color: #89ddff;
}
.code-stack .hljs-params {
  color: #eeffff;
}
.code-stack .hljs-punctuation,
.code-stack .hljs-operator {
  color: #89ddff;
}
</style>
