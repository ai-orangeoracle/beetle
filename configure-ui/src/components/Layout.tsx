import { useCallback, useContext, useEffect, useMemo, useState } from 'react'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Drawer from '@mui/material/Drawer'
import Typography from '@mui/material/Typography'
import { useTheme } from '@mui/material/styles'
import useMediaQuery from '@mui/material/useMediaQuery'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { SIDEBAR_DRAWER_BREAKPOINT } from '../config/layout'
import { ConfirmDialog } from './ConfirmDialog'
import { DeviceBanner } from './DeviceBanner'
import { Sidebar } from './Sidebar'
import { TopBar } from './TopBar'
import { NavBlockerContext } from '../contexts/NavBlockerContext'
import { UnsavedContext } from '../contexts/UnsavedContext'
import { useConfig } from '../hooks/useConfig'
import { useDeviceConnected } from '../store/deviceStatusStore'
import WarningAmberRounded from '@mui/icons-material/WarningAmberRounded'

interface LayoutProps {
  onOpenSettings?: () => void
}

export function Layout({ onOpenSettings }: LayoutProps) {
  const theme = useTheme()
  const { t } = useTranslation()
  const navigate = useNavigate()
  const location = useLocation()
  const { dirty } = useContext(UnsavedContext)
  const { config, clearCachedConfig } = useConfig()
  const deviceConnected = useDeviceConnected()
  const [pendingPath, setPendingPath] = useState<string | null>(null)
  const showDisconnectedCacheBanner = !deviceConnected && config != null
  const sidebarAsDrawer = useMediaQuery(theme.breakpoints.down(SIDEBAR_DRAWER_BREAKPOINT))
  const [drawerOpen, setDrawerOpen] = useState(false)

  const attemptNavigate = useCallback(
    (path: string) => {
      if (location.pathname === path) return
      if (!dirty) {
        navigate(path)
        return
      }
      setPendingPath(path)
    },
    [dirty, location.pathname, navigate],
  )

  const navBlockerValue = useMemo(
    () => ({ attemptNavigate }),
    [attemptNavigate],
  )

  useEffect(() => {
    if (!dirty) return
    const onBeforeUnload = (e: BeforeUnloadEvent) => {
      e.preventDefault()
    }
    window.addEventListener('beforeunload', onBeforeUnload)
    return () => window.removeEventListener('beforeunload', onBeforeUnload)
  }, [dirty])

  const showUnsavedDialog = dirty && pendingPath != null

  const handleUnsavedConfirm = useCallback(() => {
    if (pendingPath) navigate(pendingPath)
    setPendingPath(null)
  }, [navigate, pendingPath])

  return (
    <NavBlockerContext.Provider value={navBlockerValue}>
    <Box sx={{ display: 'flex', minHeight: '100vh', backgroundColor: 'var(--background)' }}>
      <ConfirmDialog
        open={showUnsavedDialog}
        onClose={() => setPendingPath(null)}
        title={t('common.unsavedLeaveTitle')}
        description={t('common.unsavedLeaveDesc')}
        icon={<WarningAmberRounded />}
        confirmColor="warning"
        confirmLabel={t('common.confirm')}
        onConfirm={handleUnsavedConfirm}
      />
      {showDisconnectedCacheBanner && (
        <Box
          role="status"
          sx={{
            flexShrink: 0,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: 2,
            px: 2,
            py: 1.25,
            borderBottom: '1px solid var(--border-subtle)',
            borderLeft: 'var(--accent-line-width, 3px) solid var(--warning)',
            backgroundColor: 'color-mix(in srgb, var(--warning) 6%, var(--surface))',
          }}
        >
          <Typography
            variant="body2"
            sx={{
              color: 'var(--warning)',
              fontWeight: 600,
              fontSize: 'var(--font-size-body-sm)',
            }}
          >
            {t('config.deviceDisconnectedCache')}
          </Typography>
          <Button
            size="small"
            variant="outlined"
            onClick={clearCachedConfig}
            sx={{
              flexShrink: 0,
              borderRadius: 'var(--radius-control)',
              borderColor: 'var(--warning)',
              color: 'var(--warning)',
            }}
          >
            {t('config.clearCache')}
          </Button>
        </Box>
      )}
      {sidebarAsDrawer ? (
        <>
          <Drawer
            anchor="left"
            open={drawerOpen}
            onClose={() => setDrawerOpen(false)}
            slotProps={{
              backdrop: { sx: { backgroundColor: 'var(--backdrop-overlay)' } },
            }}
            sx={{
              '& .MuiDrawer-paper': {
                width: 280,
                maxWidth: '85vw',
                boxSizing: 'border-box',
                backgroundColor: 'var(--card)',
                boxShadow: '8px 0 32px rgba(0,0,0,0.15)',
              },
            }}
          >
            <Sidebar drawer />
          </Drawer>
          <Box sx={{ display: 'flex', flexDirection: 'column', flex: 1, minWidth: 0 }}>
            <TopBar
              onMenuClick={() => setDrawerOpen(true)}
              onOpenSettings={() => { setDrawerOpen(false); onOpenSettings?.() }}
            />
            <DeviceBanner />
            <Box
              component="main"
              sx={{
                flex: 1,
                minHeight: 0,
                pt: 3,
                pb: 5,
                px: 2,
                width: '100%',
                backgroundColor: 'var(--surface)',
                backgroundImage: [
                  'linear-gradient(90deg, color-mix(in srgb, var(--foreground) 6%, transparent) 1px, transparent 1px)',
                  'linear-gradient(180deg, color-mix(in srgb, var(--foreground) 6%, transparent) 1px, transparent 1px)',
                ].join(', '),
                backgroundSize: '24px 24px, 24px 24px',
              }}
            >
              <Outlet />
            </Box>
          </Box>
        </>
      ) : (
        <>
          <Sidebar />
          <Box sx={{ display: 'flex', flexDirection: 'column', flex: 1, minWidth: 0 }}>
            <TopBar onOpenSettings={onOpenSettings} />
            <DeviceBanner />
            <Box
              component="main"
              sx={{
                flex: 1,
                minHeight: 0,
                overflow: 'auto',
                pt: 3,
                pb: 5,
                px: { xs: 2, md: 3 },
                width: '100%',
                backgroundColor: 'var(--surface)',
                backgroundImage: [
                  'linear-gradient(90deg, color-mix(in srgb, var(--foreground) 6%, transparent) 1px, transparent 1px)',
                  'linear-gradient(180deg, color-mix(in srgb, var(--foreground) 6%, transparent) 1px, transparent 1px)',
                ].join(', '),
                backgroundSize: '24px 24px, 24px 24px',
              }}
            >
              <Outlet />
            </Box>
          </Box>
        </>
      )}
    </Box>
    </NavBlockerContext.Provider>
  )
}
