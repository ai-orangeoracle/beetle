export interface AsyncState<T> {
  data: T
  loading: boolean
  error: string
}

export const SAVE_STATUS = ['idle', 'saving', 'ok', 'fail'] as const
export type SaveStatus = (typeof SAVE_STATUS)[number]

export function createAsyncState<T>(data: T): AsyncState<T> {
  return { data, loading: false, error: '' }
}
