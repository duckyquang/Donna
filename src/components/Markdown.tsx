import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

/**
 * Renders Donna's replies as Markdown (bold, italics, lists, headings, code, links).
 * Safe for streaming: partial Markdown renders incrementally as tokens arrive.
 * Styling lives in the `.md` class in global.css.
 */
export function Markdown({ content }: { content: string }) {
  return (
    <div className="md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ node: _node, ...props }) => (
            <a {...props} target="_blank" rel="noopener noreferrer" />
          ),
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
