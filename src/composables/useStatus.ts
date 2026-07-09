import { ref } from "vue";

export type StatusTone = "muted" | "ok" | "err";

const DEFAULT_STATUS = "系统剪贴板监听 · 文本 / 图片";

export function useStatus(defaultMessage = DEFAULT_STATUS) {
  const statusLine = ref(defaultMessage);
  const statusTone = ref<StatusTone>("muted");
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;

  function setStatus(message: string, tone: StatusTone = "muted", sticky = false) {
    statusLine.value = message;
    statusTone.value = tone;
    clearTimeout(noticeTimer);
    if (!sticky) {
      noticeTimer = setTimeout(() => {
        statusLine.value = defaultMessage;
        statusTone.value = "muted";
      }, 1600);
    }
  }

  function disposeStatus() {
    clearTimeout(noticeTimer);
  }

  return { statusLine, statusTone, setStatus, disposeStatus, defaultStatus: defaultMessage };
}
