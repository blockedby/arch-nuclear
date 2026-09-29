import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect } from 'react';

import { useCoreSetting } from './useCoreSetting';

const MINIMIZE_TO_TRAY_POLL_INTERVAL_MS = 250;

export const useTrayWindowBehavior = () => {
  const [closeToTray] = useCoreSetting<boolean>('window.closeToTray');
  const [minimizeToTray] = useCoreSetting<boolean>('window.minimizeToTray');

  useEffect(() => {
    const appWindow = getCurrentWindow();

    const unlistenClose = appWindow.onCloseRequested(async (event) => {
      if (!closeToTray) {
        return;
      }

      event.preventDefault();
      await appWindow.hide();
    });

    return () => {
      void unlistenClose.then((unlisten) => unlisten());
    };
  }, [closeToTray]);

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let isActive = true;
    let isCheckingMinimized = false;
    let hasHiddenMinimizedWindow = false;

    const hideIfMinimized = async () => {
      if (!minimizeToTray || isCheckingMinimized) {
        return;
      }

      isCheckingMinimized = true;

      try {
        const isMinimized = await appWindow.isMinimized();
        if (!isActive) {
          return;
        }

        if (!isMinimized) {
          hasHiddenMinimizedWindow = false;
          return;
        }

        if (hasHiddenMinimizedWindow) {
          return;
        }

        await appWindow.hide();
        hasHiddenMinimizedWindow = true;
      } finally {
        isCheckingMinimized = false;
      }
    };

    const unlistenResize = appWindow.onResized(hideIfMinimized);
    const minimizePollingInterval = minimizeToTray
      ? window.setInterval(
          () => void hideIfMinimized(),
          MINIMIZE_TO_TRAY_POLL_INTERVAL_MS,
        )
      : undefined;

    return () => {
      isActive = false;

      if (minimizePollingInterval !== undefined) {
        window.clearInterval(minimizePollingInterval);
      }

      void unlistenResize.then((unlisten) => unlisten());
    };
  }, [minimizeToTray]);
};
