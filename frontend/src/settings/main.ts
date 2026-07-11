import { createApp } from 'vue';
import App from './App.vue';
import '@/styles/main.css';
import '@/popup/popup-tokens.css';
import '@/popup/components.css';
import { initializeI18n } from '@/i18n'
import { createLogger } from '@public/logger.js'

const logger = createLogger('settings')

try {
  await initializeI18n()
} catch (error) {
  logger.warn('界面语言初始化失败，使用当前可用语言挂载', String(error))
}

createApp(App).mount('#app');
