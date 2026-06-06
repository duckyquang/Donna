import { Markdown } from "./Markdown";
import { DonnaQuestionBlock, DonnaQuestionPending } from "./DonnaQuestion";
import { parseDonnaMessage, parseDonnaMessageStreaming } from "../lib/donnaQuestions";

interface DonnaMessageProps {
  content: string;
  /** Assistant reply is still streaming from the model. */
  streaming?: boolean;
  onAnswer?: (answer: string) => void;
}

export function DonnaMessage({ content, streaming = false, onAnswer }: DonnaMessageProps) {
  const { segments, pendingQuestion } = streaming
    ? parseDonnaMessageStreaming(content)
    : { segments: parseDonnaMessage(content), pendingQuestion: false };

  return (
    <div>
      {segments.map((seg, i) =>
        seg.kind === "markdown" ? (
          <Markdown key={`md-${i}`} content={seg.text} />
        ) : (
          <DonnaQuestionBlock
            key={`q-${i}`}
            question={seg.question}
            onAnswer={onAnswer ?? (() => {})}
            disabled={streaming || !onAnswer}
          />
        )
      )}
      {streaming && pendingQuestion && <DonnaQuestionPending />}
    </div>
  );
}
