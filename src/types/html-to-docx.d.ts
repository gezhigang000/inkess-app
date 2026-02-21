declare module 'html-to-docx' {
  interface Options {
    title?: string
    margins?: { top?: number; right?: number; bottom?: number; left?: number }
  }
  export default function HTMLtoDOCX(
    html: string,
    headerHtml?: string,
    options?: Options,
  ): Promise<Blob>
}
