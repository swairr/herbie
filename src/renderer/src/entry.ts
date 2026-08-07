import { createApp } from 'vue'
import './style.css'
import App from './App.vue'

createApp(App).mount('#app')

if (import.meta.env.DEV && (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) {
  void import('./components/TauriPingDemo.vue').then(({ default: TauriPingDemo }) => {
    const host = document.createElement('div')
    document.body.appendChild(host)
    createApp(TauriPingDemo).mount(host)
  })
}