import { useEffect, useMemo, useState } from "react";
import { Send } from "lucide-react";
import { Markdown } from "./Markdown";
import { DonnaQuestionBlock, DonnaQuestionPending } from "./DonnaQuestion";
import {
  formatNumberedAnswers,
  parseDonnaMessage,
  parseDonnaMessageStreaming,
  questionSegments,
  type DonnaQuestion,
} from "../lib/donnaQuestions";

interface DonnaMessageProps {
  content: string;
  streaming?: boolean;
  onAnswer?: (answer: string) => void;
}

function isAnswerComplete(question: DonnaQuestion, answer: string): boolean {
  const trimmed = answer.trim();
  if (!trimmed) return false;
  if (question.type === "choice" && question.options?.some((o) => o.toLowerCase() === "other")) {
    return trimmed.length > 0;
  }
  return true;
}

export function DonnaMessage({ content, streaming = false, onAnswer }: DonnaMessageProps) {
  const { segments, pendingQuestion } = streaming
    ? parseDonnaMessageStreaming(content)
    : { segments: parseDonnaMessage(content), pendingQuestion: false };

  const questions = useMemo(() => questionSegments(content), [content]);
  const batchMode = questions.length > 1 && !streaming && !!onAnswer;

  const [answers, setAnswers] = useState<string[]>(() => questions.map(() => ""));
  const [batchSubmitted, setBatchSubmitted] = useState(false);

  const questionKey = questions.map((q) => q.prompt).join("\0");
  useEffect(() => {
    setAnswers(questions.map(() => ""));
    setBatchSubmitted(false);
  }, [questionKey, questions.length]);

  let questionIndex = -1;

  const allComplete =
    batchMode &&
    questions.every((q, i) => isAnswerComplete(q, answers[i] ?? ""));

  const submitBatch = () => {
    if (!allComplete || batchSubmitted || !onAnswer) return;
    onAnswer(formatNumberedAnswers(answers));
    setBatchSubmitted(true);
  };

  const setAnswerAt = (index: number, value: string) => {
    setAnswers((prev) => {
      const next = [...prev];
      next[index] = value;
      return next;
    });
  };

  return (
    <div>
      {segments.map((seg, i) => {
        if (seg.kind === "markdown") {
          return <Markdown key={`md-${i}`} content={seg.text} />;
        }

        questionIndex += 1;
        const qIndex = questionIndex;

        return (
          <DonnaQuestionBlock
            key={`q-${i}`}
            question={seg.question}
            batchMode={batchMode}
            batchIndex={batchMode ? qIndex : undefined}
            value={answers[qIndex] ?? ""}
            onChange={(v) => setAnswerAt(qIndex, v)}
            onAnswer={batchMode ? undefined : onAnswer}
            disabled={streaming || !onAnswer}
            submitted={batchSubmitted}
          />
        );
      })}

      {batchMode && (
        <button
          type="button"
          onClick={submitBatch}
          disabled={!allComplete || batchSubmitted}
          className="mt-3 flex w-full items-center justify-center gap-2 rounded-lg bg-donna-accent py-2.5 text-sm font-medium text-white hover:bg-donna-accent-hover disabled:opacity-40"
        >
          <Send size={14} />
          {batchSubmitted ? "Answers sent" : "Submit all answers"}
        </button>
      )}

      {streaming && pendingQuestion && <DonnaQuestionPending />}
    </div>
  );
}
