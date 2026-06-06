import { Markdown } from "./Markdown";
import { DonnaQuestionBlock } from "./DonnaQuestion";
import { parseDonnaMessage } from "../lib/donnaQuestions";

interface DonnaMessageProps {
  content: string;
  /** When false, question widgets stay hidden until the block is complete (streaming). */
  interactive?: boolean;
  onAnswer?: (answer: string) => void;
}

export function DonnaMessage({ content, interactive = true, onAnswer }: DonnaMessageProps) {
  const segments = parseDonnaMessage(content);

  return (
    <div>
      {segments.map((seg, i) =>
        seg.kind === "markdown" ? (
          <Markdown key={`md-${i}`} content={seg.text} />
        ) : interactive && onAnswer ? (
          <DonnaQuestionBlock
            key={`q-${i}`}
            question={seg.question}
            onAnswer={onAnswer}
          />
        ) : null
      )}
    </div>
  );
}
