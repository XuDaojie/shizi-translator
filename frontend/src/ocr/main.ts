import { createApp } from 'vue'
import OcrWindow from './OcrWindow.vue'
import '@/styles/main.css'
import { createLogger } from '@public/logger.js'

const logger = createLogger('ocr')
logger.info('OCR 窗口前端挂载')

createApp(OcrWindow).mount('#app')
