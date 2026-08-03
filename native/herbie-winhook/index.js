'use strict'

let native
try {
  // Built by node-gyp / electron-rebuild into build/Release.
  native = require('./build/Release/herbie_winhook.node')
} catch (e) {
  native = null
}

// Graceful fallback: when the native module is not built (e.g. missing VS Build Tools
// or running on non-Windows), startTracking receives a no-op notifier and the app
// continues to work without segment recording. The C++ side never holds business logic.
module.exports = {
  start(cb) {
    if (!native) return
    return native.start(cb)
  },
  stop() {
    if (!native) return
    return native.stop()
  }
}