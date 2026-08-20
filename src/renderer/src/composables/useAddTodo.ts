import { ref } from 'vue'
import type { Ref } from 'vue'
import type { Todo } from '@shared/types'

export function useAddTodo(title: Ref<string>, detail: Ref<string>) {
  const adding = ref(false)
  const shaking = ref(false)

  async function submit(): Promise<Todo | null> {
    const t = title.value.trim()
    if (!t) {
      shaking.value = true
      setTimeout(() => (shaking.value = false), 400)
      return null
    }
    if (adding.value) return null
    adding.value = true
    try {
      const todo = await window.api.todos.create({ title: t, detail: detail.value })
      title.value = ''
      detail.value = ''
      return todo
    } finally {
      adding.value = false
    }
  }

  return { adding, shaking, submit }
}
