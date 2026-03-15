import { createContext } from 'react'

export interface NavBlockerContextValue {
  /** 尝试导航到 path；若有未保存修改会先弹窗，确认后再跳转 */
  attemptNavigate: (path: string) => void
}

export const NavBlockerContext = createContext<NavBlockerContextValue | null>(null)
