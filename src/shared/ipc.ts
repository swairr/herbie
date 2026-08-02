export const IPC = {
  todos: {
    list: 'todos:list',
    create: 'todos:create',
    update: 'todos:update',
    toggle: 'todos:toggle',
    softDelete: 'todos:softDelete',
    labels: 'todos:labels'
  },
  settings: {
    get: 'settings:get',
    set: 'settings:set',
    getAll: 'settings:getAll'
  },
  export: {
    exportMarkdown: 'export:exportMarkdown'
  },
  shell: {
    openExternal: 'shell:openExternal'
  },
  clipboard: {
    readText: 'clipboard:readText'
  },
  window: {
    quickAddHide: 'window:quickAddHide',
    quickAddFocus: 'window:quickAddFocus'
  },
  shortcut: {
    error: 'shortcut:error'
  },
  dialog: {
    pickDirectory: 'dialog:pickDirectory'
  },
  quickadd: {
    showEvent: 'quickadd:show',
    hideEvent: 'quickadd:hide',
    blurEvent: 'quickadd:blur'
  }
} as const