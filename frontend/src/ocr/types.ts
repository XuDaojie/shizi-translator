/** OCR 识别结果元信息（与后端 OcrRunMeta camelCase 对齐）。 */
export interface OcrRunMeta {
  engine: string
  model?: string | null
  sourceWidth: number
  sourceHeight: number
  sentWidth: number
  sentHeight: number
  pngBytes?: number | null
  latencyMs: number
  httpStatus?: number | null
  scaled: boolean
  /** PDF 打开路径：识别的源页码（1-based）；图片路径可缺省 */
  sourcePage?: number | null
  /** PDF 打开路径：文档总页数；图片路径可缺省 */
  sourcePageCount?: number | null
}

/** 纯识别完整响应（与后端 RecognizeImageResponse camelCase 对齐）。 */
export interface RecognizeImageResponse {
  text: string
  meta: OcrRunMeta
  /** 供 UI 预览的 PNG base64（无 data: 前缀） */
  previewPngBase64: string
}

export type OcrStatus = 'idle' | 'loading' | 'success' | 'error'
