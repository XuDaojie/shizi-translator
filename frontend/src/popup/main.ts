import { createApp } from 'vue'
import TranslationPopup from './TranslationPopup.vue'
import '@/popup/popup-tokens.css'
import '@/popup/index.css'
import '@/popup/components.css'
import { initializeI18n } from '@/i18n'
import { createLogger } from '@public/logger.js'

const logger = createLogger('translate')

try {
  await initializeI18n()
} catch (error) {
  logger.warn('界面语言初始化失败，使用当前可用语言挂载', String(error))
}

createApp(TranslationPopup).mount('#app')
