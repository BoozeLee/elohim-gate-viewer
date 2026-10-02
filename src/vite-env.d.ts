/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Set to "1" to run the gate as soon as the window opens. See src/main.ts. */
  readonly VITE_AUTORUN?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}