import { renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { registerBuiltInCoreSettings } from '../services/coreSettings';
import { initializeSettingsStore, setSetting } from '../stores/settingsStore';
import { resetInMemoryTauriStore } from '../test/utils/inMemoryTauriStore';
import { useTrayWindowBehavior } from './useTrayWindowBehavior';

const windowMock = vi.hoisted(() => ({
  closeHandler: undefined as
    | undefined
    | ((event: { preventDefault: () => void }) => Promise<void>),
  resizeHandler: undefined as undefined | (() => Promise<void>),
  hide: vi.fn(),
  isMinimized: vi.fn(),
  onCloseRequested: vi.fn((handler) => {
    windowMock.closeHandler = handler;
    return Promise.resolve(vi.fn());
  }),
  onResized: vi.fn((handler) => {
    windowMock.resizeHandler = handler;
    return Promise.resolve(vi.fn());
  }),
  setDecorations: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => windowMock,
}));

const MINIMIZE_TO_TRAY_POLL_INTERVAL_MS = 250;

const mockMinimizePollingInterval = () => {
  const originalSetInterval = window.setInterval.bind(window);
  let pollingHandler: (() => void) | undefined;

  vi.spyOn(window, 'setInterval').mockImplementation((handler, timeout) => {
    if (
      timeout === MINIMIZE_TO_TRAY_POLL_INTERVAL_MS &&
      typeof handler === 'function'
    ) {
      pollingHandler = handler as () => void;
      return 1;
    }

    return originalSetInterval(handler, timeout);
  });

  return {
    get pollingHandler() {
      return pollingHandler;
    },
  };
};

beforeEach(async () => {
  vi.clearAllMocks();
  windowMock.closeHandler = undefined;
  windowMock.resizeHandler = undefined;
  windowMock.hide.mockResolvedValue(undefined);
  windowMock.isMinimized.mockResolvedValue(false);
  resetInMemoryTauriStore();
  await initializeSettingsStore();
  registerBuiltInCoreSettings();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

it('hides the window on close by default', async () => {
  renderHook(() => useTrayWindowBehavior());

  await waitFor(() => expect(windowMock.closeHandler).toBeDefined());
  const preventDefault = vi.fn();
  await windowMock.closeHandler?.({ preventDefault });

  expect(preventDefault).toHaveBeenCalledTimes(1);
  expect(windowMock.hide).toHaveBeenCalledTimes(1);
});

it('does not intercept close or minimize when tray settings are disabled', async () => {
  await setSetting('core.window.closeToTray', false);

  renderHook(() => useTrayWindowBehavior());

  await waitFor(() => expect(windowMock.closeHandler).toBeDefined());
  const preventDefault = vi.fn();
  await windowMock.closeHandler?.({ preventDefault });
  windowMock.isMinimized.mockResolvedValue(true);
  await windowMock.resizeHandler?.();

  expect(preventDefault).not.toHaveBeenCalled();
  expect(windowMock.hide).not.toHaveBeenCalled();
});

it('hides the window after minimize when minimize to tray is enabled', async () => {
  await setSetting('core.window.minimizeToTray', true);
  windowMock.isMinimized.mockResolvedValue(true);

  renderHook(() => useTrayWindowBehavior());

  await waitFor(() => expect(windowMock.resizeHandler).toBeDefined());
  await windowMock.resizeHandler?.();

  expect(windowMock.hide).toHaveBeenCalledTimes(1);
});

it('polls for minimized state and hides when minimize to tray is enabled', async () => {
  await setSetting('core.window.minimizeToTray', true);
  windowMock.isMinimized.mockResolvedValue(true);
  const minimizePolling = mockMinimizePollingInterval();

  const { unmount } = renderHook(() => useTrayWindowBehavior());

  await waitFor(() => expect(minimizePolling.pollingHandler).toBeDefined());
  minimizePolling.pollingHandler?.();

  await waitFor(() => expect(windowMock.hide).toHaveBeenCalledTimes(1));

  unmount();
});

it('does not repeatedly hide while minimized polling is already handled', async () => {
  await setSetting('core.window.minimizeToTray', true);
  windowMock.isMinimized.mockResolvedValue(true);
  let completeHide: () => void;
  windowMock.hide.mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        completeHide = resolve;
      }),
  );
  const minimizePolling = mockMinimizePollingInterval();

  const { unmount } = renderHook(() => useTrayWindowBehavior());

  await waitFor(() => expect(minimizePolling.pollingHandler).toBeDefined());
  minimizePolling.pollingHandler?.();
  minimizePolling.pollingHandler?.();
  minimizePolling.pollingHandler?.();

  await waitFor(() => expect(windowMock.hide).toHaveBeenCalledTimes(1));

  minimizePolling.pollingHandler?.();
  expect(windowMock.hide).toHaveBeenCalledTimes(1);

  completeHide!();
  minimizePolling.pollingHandler?.();

  await waitFor(() => expect(windowMock.hide).toHaveBeenCalledTimes(1));

  unmount();
});

it('does not poll-hide when minimize to tray is disabled', async () => {
  windowMock.isMinimized.mockResolvedValue(true);
  const minimizePolling = mockMinimizePollingInterval();

  const { unmount } = renderHook(() => useTrayWindowBehavior());

  await waitFor(() => expect(windowMock.resizeHandler).toBeDefined());

  expect(minimizePolling.pollingHandler).not.toBeDefined();
  expect(windowMock.hide).not.toHaveBeenCalled();

  unmount();
});
