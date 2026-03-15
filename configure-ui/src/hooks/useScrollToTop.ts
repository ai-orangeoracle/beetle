import { useEffect } from 'react'
import { useLocation } from 'react-router-dom'

/** 路由切换时滚动到顶部，需在 Router 内调用 */
export function useScrollToTop() {
  const { pathname } = useLocation()
  useEffect(() => {
    window.scrollTo(0, 0)
  }, [pathname])
}
