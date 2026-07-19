import { createApp } from 'vue'
import TranslationPopup from './TranslationPopup.vue'
import '@/popup/popup-tokens.css'
import '@/popup/index.css'
import '@/popup/components.css'
import { initializeI18n } from '@/i18n'
import { createLogger } from '@public/logger.js'
import { dismissBootSplash } from '@/shared/bootSplash'

const logger = createLogger('translate')

let initializationTimer: ReturnType<typeof setTimeout> | undefined
const initialization = initializeI18n().catch((error) => {
  logger.warn('界面语言初始化失败，使用当前可用语言挂载', String(error))
})
await Promise.race([
  initialization,
  new Promise<void>((resolve) => {
    initializationTimer = setTimeout(() => {
      logger.warn('界面语言初始化超时，使用当前可用语言挂载')
      resolve()
    }, 1000)
  }),
])
if (initializationTimer !== undefined) clearTimeout(initializationTimer)

createApp(TranslationPopup).mount('#app')
void dismissBootSplash()
