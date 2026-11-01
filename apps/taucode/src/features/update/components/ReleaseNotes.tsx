import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

function openExternal(href: string) {
  void import("@tauri-apps/plugin-opener").then(({ openUrl }) => openUrl(href)).catch(() => {});
}

export function ReleaseNotes({ notes, className }: { notes: string; className?: string }) {
  return (
    <div className={className} role="document" aria-label="Release notes">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children }) => href?.startsWith("https://")
            ? <a href={href} target="_blank" rel="noreferrer" onClick={event => { event.preventDefault(); openExternal(href); }}>{children}</a>
            : <span>{children}</span>,
          img: () => null,
        }}
      >
        {notes}
      </ReactMarkdown>
    </div>
  );
}
