import { createApp } from 'vue'
import './style.css'
import App from './App.vue'

// 在 Tauri 环境(`pnpm tauri dev`/打包运行)挂载前注入形状等价的薄封装 `window.api`;
// 非 Tauri 环境(纯浏览器构建)标志为 false,不注入、直接挂载。
// 用 then 链而非 top-level await,以兼容 vite 默认构建目标(es2020),并在 mount 之前完成注入。
const isTauri = !!(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__

function mountApp(): void {
  createApp(App).mount('#app')
}

if (isTauri) {
  void Promise.all([import('./api/tauri'), import('@tauri-apps/api/window')]).then(
    ([{ createTauriApi }, { getCurrentWindow }]) => {
      // Quick Add 窗口(label === 'quickadd')由 Rust 用默认 URL 创建(不带 hash),
      // 这里按窗口 label 重定向到 App.vue 的 `#/quickadd` 路由。
      if (getCurrentWindow().label === 'quickadd') {
        location.hash = '#/quickadd'
      }
      window.api = createTauriApi()
      mountApp()
    }
  )
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