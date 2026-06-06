import { useState } from "react";
import { Send } from "lucide-react";
import type { DonnaQuestion as Q } from "../lib/donnaQuestions";
import { ThinkingDots } from "./ui";

interface DonnaQuestionProps {
  question: Q;
  /** Standalone mode: submit sends one answer immediately. */
  onAnswer?: (answer: string) => void;
  /** Batch mode: report answer changes to the parent form. */
  onChange?: (answer: string) => void;
  value?: string;
  batchMode?: boolean;
  batchIndex?: number;
  disabled?: boolean;
  submitted?: boolean;
}

/** Shown under streamed text while a donna-ask block is still being generated. */
export function DonnaQuestionPending() {
  return (
    <div className="mt-3 flex items-center gap-2 rounded-xl border border-white/10 bg-donna-bg/50 px-4 py-3">
      <ThinkingDots />
      <span className="text-xs text-gray-500">Preparing a question…</span>
    </div>
  );
}

export function DonnaQuestionBlock({
  question,
  onAnswer,
  onChange,
  value = "",
  batchMode = false,
  batchIndex,
  disabled,
  submitted = false,
}: DonnaQuestionProps) {
  const [selected, setSelected] = useState<string | null>(null);
  const [otherText, setOtherText] = useState("");
  const [textAnswer, setTextAnswer] = useState(value);
  const [standaloneAnswered, setStandaloneAnswered] = useState(false);

  const locked = disabled || submitted || (!batchMode && standaloneAnswered);

  const reportBatch = (answer: string) => {
    onChange?.(answer);
  };

  const pickChoice = (opt: string, otherValue: string) => {
    setSelected(opt);
    if (!batchMode) return;
    if (opt.toLowerCase() === "other") {
      reportBatch(otherValue.trim());
    } else {
      reportBatch(opt);
    }
  };

  const submitChoice = () => {
    if (!selected || locked) return;
    const answer =
      selected.toLowerCase() === "other" ? otherText.trim() : selected;
    if (!answer) return;
    if (batchMode) {
      reportBatch(answer);
      return;
    }
    setStandaloneAnswered(true);
    onAnswer?.(answer);
  };

  const submitText = () => {
    const answer = textAnswer.trim();
    if (!answer || locked) return;
    if (batchMode) {
      reportBatch(answer);
      return;
    }
    setStandaloneAnswered(true);
    onAnswer?.(answer);
  };

  const handleTextChange = (next: string) => {
    setTextAnswer(next);
    if (batchMode) reportBatch(next.trim());
  };

  const handleOtherChange = (next: string) => {
    setOtherText(next);
    if (batchMode && selected?.toLowerCase() === "other") {
      reportBatch(next.trim());
    }
  };

  const showStandaloneSubmit = !batchMode;
  const showBatchDone = batchMode && submitted && value.trim();

  return (
    <div className="mt-3 rounded-xl border border-donna-accent/25 bg-donna-bg/80 p-4">
      <p className="mb-3 text-sm font-medium text-white">
        {batchMode && batchIndex !== undefined && (
          <span className="mr-2 text-donna-accent-light">{batchIndex + 1}.</span>
        )}
        {question.prompt}
      </p>

      {question.type === "choice" && question.options && (
        <div className="space-y-2">
          {question.options.map((opt) => {
            const isOther = opt.toLowerCase() === "other";
            const isActive = selected === opt;
            return (
              <div key={opt}>
                <button
                  type="button"
                  disabled={locked}
                  onClick={() => pickChoice(opt, otherText)}
                  className={`w-full rounded-lg border px-3 py-2 text-left text-sm transition-colors ${
                    isActive
                      ? "border-donna-accent bg-donna-accent/15 text-donna-accent-light"
                      : "border-white/10 text-gray-300 hover:border-white/20 hover:bg-white/5"
                  } disabled:opacity-50`}
                >
                  {opt}
                </button>
                {isOther && isActive && (
                  <input
                    value={otherText}
                    onChange={(e) => handleOtherChange(e.target.value)}
                    disabled={locked}
                    placeholder="Type your answer…"
                    className="mt-2 w-full rounded-lg border border-white/10 bg-donna-surface px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !batchMode) {
                        e.preventDefault();
                        submitChoice();
                      }
                    }}
                  />
                )}
              </div>
            );
          })}
          {showStandaloneSubmit && (
            <button
              type="button"
              onClick={submitChoice}
              disabled={
                locked ||
                !selected ||
                (selected.toLowerCase() === "other" && !otherText.trim())
              }
              className="mt-1 flex w-full items-center justify-center gap-2 rounded-lg bg-donna-accent py-2 text-sm font-medium text-white hover:bg-donna-accent-hover disabled:opacity-40"
            >
              <Send size={14} />
              Submit answer
            </button>
          )}
        </div>
      )}

      {question.type === "text" && (
        <div className="flex gap-2">
          <input
            value={batchMode ? value || textAnswer : textAnswer}
            onChange={(e) => handleTextChange(e.target.value)}
            disabled={locked}
            placeholder="Your answer…"
            className="flex-1 rounded-lg border border-white/10 bg-donna-surface px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
            onKeyDown={(e) => {
              if (e.key === "Enter" && !batchMode) {
                e.preventDefault();
                submitText();
              }
            }}
          />
          {showStandaloneSubmit && (
            <button
              type="button"
              onClick={submitText}
              disabled={locked || !textAnswer.trim()}
              className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-donna-accent text-white hover:bg-donna-accent-hover disabled:opacity-40"
            >
              <Send size={16} />
            </button>
          )}
        </div>
      )}

      {!batchMode && standaloneAnswered && (
        <p className="mt-2 text-xs text-gray-500">Answer sent — Donna will remember.</p>
      )}
      {showBatchDone && (
        <p className="mt-2 text-xs text-gray-500">{value}</p>
      )}
    </div>
  );
}
