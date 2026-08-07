import { createApp } from 'vue'
import './style.css'
import App from './App.vue'

// Tauri 环境下(`pnpm tauri dev`)在挂载前注入形状等价的薄封装 `window.api`;
// Electron 路径(`pnpm dev`/`pnpm build`)由 preload 提供 `window.api`,此处标志为 false 不污染。
// 用 then 链而非 top-level await,以兼容 vite 默认构建目标(es2020),并在 mount 之前完成注入。
const isTauri = !!(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__

function mountApp(): void {
  createApp(App).mount('#app')
}

if (isTauri) {
  void import('./api/tauri').then(({ createTauriApi }) => {
    window.api = createTauriApi()
    mountApp()
  })
} else {
  mountApp()
}

if (import.meta.env.DEV && isTauri) {
  void import('./components/TauriPingDemo.vue').then(({ default: TauriPingDemo }) => {
    const host = document.createElement('div')
    document.body.appendChild(host)
    createApp(TauriPingDemo).mount(host)
  })
}