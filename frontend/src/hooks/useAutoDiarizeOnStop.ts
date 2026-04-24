'use client';

import { useEffect, useState } from 'react';

const STORE_FILE = 'preferences.json';
const KEY_ENABLED = 'diarization.auto_on_stop';

async function readEnabled(): Promise<boolean> {
  try {
    const { Store } = await import('@tauri-apps/plugin-store');
    const store = await Store.load(STORE_FILE);
    return (await store.get<boolean>(KEY_ENABLED)) ?? false;
  } catch {
    return false;
  }
}

export async function setAutoDiarizeOnStop(enabled: boolean): Promise<void> {
  const { Store } = await import('@tauri-apps/plugin-store');
  const store = await Store.load(STORE_FILE);
  await store.set(KEY_ENABLED, enabled);
  await store.save();
  window.dispatchEvent(new CustomEvent('auto-diarize-pref-changed'));
}

export function useAutoDiarizeOnStop(): [boolean, (v: boolean) => Promise<void>] {
  const [enabled, setEnabled] = useState(false);

  useEffect(() => {
    let disposed = false;
    const refresh = async () => {
      const v = await readEnabled();
      if (!disposed) setEnabled(v);
    };
    const handler = () => void refresh();
    window.addEventListener('auto-diarize-pref-changed', handler);
    void refresh();
    return () => {
      disposed = true;
      window.removeEventListener('auto-diarize-pref-changed', handler);
    };
  }, []);

  const update = async (v: boolean) => {
    setEnabled(v);
    try {
      await setAutoDiarizeOnStop(v);
    } catch (err) {
      console.error('Failed to persist auto-diarize preference:', err);
      // Revert local state on failure
      const actual = await readEnabled();
      setEnabled(actual);
    }
  };

  return [enabled, update];
}
