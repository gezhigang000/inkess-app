export type ContentData =
  | { type: 'markdown'; text: string }
  | { type: 'text'; text: string }
  | { type: 'code'; text: string; language: string }
  | { type: 'html'; text: string }
  | { type: 'image'; src: string }
  | { type: 'pdf'; src: string; data?: Uint8Array }
  | { type: 'docx'; html: string }
  | { type: 'xlsx'; sheets: SheetData[] }

export interface SheetData {
  name: string
  rows: string[][]
}
