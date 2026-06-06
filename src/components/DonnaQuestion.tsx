import { useState } from "react";
import { Send } from "lucide-react";
import type { DonnaQuestion as Q } from "../lib/donnaQuestions";

interface DonnaQuestionProps {
  question: Q;
  onAnswer: (answer: string) => void;
  disabled?: boolean;
}

export function DonnaQuestionBlock({ question, onAnswer, disabled }: DonnaQuestionProps) {
  const [selected, setSelected] = useState<string | null>(null);
  const [otherText, setOtherText] = useState("");
  const [textAnswer, setTextAnswer] = useState("");
  const [answered, setAnswered] = useState(false);

  const locked = disabled || answered;

  const submitChoice = () => {
    if (!selected || locked) return;
    const answer =
      selected.toLowerCase() === "other" ? otherText.trim() : selected;
    if (!answer) return;
    setAnswered(true);
    onAnswer(answer);
  };

  const submitText = () => {
    const answer = textAnswer.trim();
    if (!answer || locked) return;
    setAnswered(true);
    onAnswer(answer);
  };

  return (
    <div className="mt-3 rounded-xl border border-donna-accent/25 bg-donna-bg/80 p-4">
      <p className="mb-3 text-sm font-medium text-white">{question.prompt}</p>

      {question.type === "choice" && question.options && (
        <div className="space-y-2">
          {question.options.map((opt) => {
            const isOther = opt.toLowerCase() === "other";
            const active = selected === opt;
            return (
              <div key={opt}>
                <button
                  type="button"
                  disabled={locked}
                  onClick={() => setSelected(opt)}
                  className={`w-full rounded-lg border px-3 py-2 text-left text-sm transition-colors ${
                    active
                      ? "border-donna-accent bg-donna-accent/15 text-donna-accent-light"
                      : "border-white/10 text-gray-300 hover:border-white/20 hover:bg-white/5"
                  } disabled:opacity-50`}
                >
                  {opt}
                </button>
                {isOther && active && (
                  <input
                    value={otherText}
                    onChange={(e) => setOtherText(e.target.value)}
                    disabled={locked}
                    placeholder="Type your answer…"
                    className="mt-2 w-full rounded-lg border border-white/10 bg-donna-surface px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        submitChoice();
                      }
                    }}
                  />
                )}
              </div>
            );
          })}
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
        </div>
      )}

      {question.type === "text" && (
        <div className="flex gap-2">
          <input
            value={textAnswer}
            onChange={(e) => setTextAnswer(e.target.value)}
            disabled={locked}
            placeholder="Your answer…"
            className="flex-1 rounded-lg border border-white/10 bg-donna-surface px-3 py-2 text-sm text-white outline-none focus:border-donna-accent"
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                submitText();
              }
            }}
          />
          <button
            type="button"
            onClick={submitText}
            disabled={locked || !textAnswer.trim()}
            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-donna-accent text-white hover:bg-donna-accent-hover disabled:opacity-40"
          >
            <Send size={16} />
          </button>
        </div>
      )}

      {answered && (
        <p className="mt-2 text-xs text-gray-500">Answer sent — Donna will remember.</p>
      )}
    </div>
  );
}
