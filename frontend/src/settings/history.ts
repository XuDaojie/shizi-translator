import {
  invokeClearTranslationHistory,
  invokeListTranslationHistory,
  type HistoryResultDto,
  type HistorySessionDto,
  type HistoryTrigger,
} from '@/lib/tauri'

export type HistoryResult = HistoryResultDto
export type HistorySession = HistorySessionDto
export type { HistoryTrigger }

export type ResultCardStatus = 'success' | 'loading' | 'pending' | 'error' | 'aborted'

export const isEmptyHistory = (sessions: HistorySession[]): boolean => sessions.length === 0

export const loadHistory = (limit?: number): Promise<HistorySession[]> =>
  invokeListTranslationHistory(limit)

export const clearHistoryAndReload = async (): Promise<HistorySession[]> => {
  await invokeClearTranslationHistory()
  return loadHistory()
}

export const resultCardStatus = (result: HistoryResult): ResultCardStatus => {
  if (result.status === 'cancelled') return 'aborted'
  return result.status
}
