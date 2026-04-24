'use client';

import { useEffect, useState } from 'react';

const STORE_FILE = 'preferences.json';
const STORE_KEY = 'user.displayName';

type Unlistener = () => void;

async function readDisplayName(): Promise<string> {
  try {
    const { Store } = await import('@tauri-apps/plugin-store');
    const store = await Store.load(STORE_FILE);
    const value = await store.get<string>(STORE_KEY);
    return (value ?? '').trim();
  } catch {
    return '';
  }
}

export async function setUserDisplayName(name: string): Promise<void> {
  const { Store } = await import('@tauri-apps/plugin-store');
  const store = await Store.load(STORE_FILE);
  await store.set(STORE_KEY, name.trim());
  await store.save();
  window.dispatchEvent(new CustomEvent('user-display-name-changed'));
}

export function useUserDisplayName(): string {
  const [name, setName] = useState<string>('');

  useEffect(() => {
    let disposed = false;
    let unlistener: Unlistener | null = null;

    const load = async () => {
      const v = await readDisplayName();
      if (!disposed) setName(v);
    };

    const handler = () => {
      void load();
    };

    window.addEventListener('user-display-name-changed', handler);
    unlistener = () => window.removeEventListener('user-display-name-changed', handler);

    void load();

    return () => {
      disposed = true;
      unlistener?.();
    };
  }, []);

  return name;
}
