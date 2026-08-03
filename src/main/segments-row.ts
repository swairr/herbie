import type { Segment, SegmentKind } from '@shared/types'

export interface SegmentRow {
  id: string
  startAt: string
  endAt: string | null
  processName: string
  title: string
  note: string
  todoId: string | null
  kind: string
}

// Single row->Segment mapper shared by the writer (segments.ts) and the reader
// (segments-query.ts) so a schema change cannot drift them apart.
export function rowToSegment(r: SegmentRow): Segment {
  return {
    id: r.id,
    startAt: r.startAt,
    endAt: r.endAt,
    processName: r.processName,
    title: r.title,
    note: r.note,
    todoId: r.todoId,
    kind: (r.kind as SegmentKind) ?? 'activity'
  }
}