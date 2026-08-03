export interface WinHookEvent {
  type: 'foreground' | 'namechange'
  hwnd: number
  processName: string
  title: string
}

export type WinHookCallback = (event: WinHookEvent) => void

export interface WinHookModule {
  start(cb: WinHookCallback): void
  stop(): void
}

declare const exports: WinHookModule
export default exports