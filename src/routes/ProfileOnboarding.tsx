import { useMemo, useState } from "react";
import { ArrowLeft, ArrowRight, Check, Sparkles } from "lucide-react";
import { api } from "../lib/api";
import { useConfig } from "../lib/useConfig";
import { Button, Spinner } from "../components/ui";

type WorkStudy = "work" | "study" | "both" | "neither";

interface ProfileAnswers {
  name: string;
  age: string;
  nationality: string;
  birthday: string;
  location: string;
  workStudy: WorkStudy;
  workStudyDetail: string;
}

const AGE_OPTIONS = [
  "Under 18",
  "18–24",
  "25–34",
  "35–44",
  "45–54",
  "55+",
  "Prefer not to say",
] as const;

const STEPS = [
  { id: "welcome", title: "Welcome" },
  { id: "name", title: "Your name" },
  { id: "age", title: "Age" },
  { id: "nationality", title: "Nationality" },
  { id: "birthday", title: "Birthday" },
  { id: "location", title: "Location" },
  { id: "work", title: "Work or study" },
] as const;

function detectedTimezone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone;
  } catch {
    return "";
  }
}

/** Pull an organization/school name from free-text for a sub-folder branch. */
function branchFromDetail(detail: string): string {
  const t = detail.trim();
  if (!t) return "General";
  const atMatch = t.match(/\bat\s+([^,;]+)/i);
  if (atMatch) return atMatch[1]!.trim();
  return t.split(/[,;]/)[0]!.trim().slice(0, 48);
}

async function saveProfileToKnowledge(answers: ProfileAnswers) {
  const saves: Promise<unknown>[] = [];

  if (answers.name.trim()) {
    saves.push(
      api.kgSaveNode({
        folder: ["About You", "Identity"],
        label: "Preferred name",
        note: answers.name.trim(),
        type: "info",
      })
    );
  }

  if (answers.age && answers.age !== "Prefer not to say") {
    saves.push(
      api.kgSaveNode({
        folder: ["About You", "Identity"],
        label: "Age",
        note: answers.age,
        type: "info",
      })
    );
  }

  if (answers.nationality.trim()) {
    saves.push(
      api.kgSaveNode({
        folder: ["About You", "Nationality"],
        label: "Country",
        note: answers.nationality.trim(),
        type: "info",
      })
    );
  }

  if (answers.birthday) {
    saves.push(
      api.kgSaveNode({
        folder: ["About You", "Identity"],
        label: "Birthday",
        note: answers.birthday,
        type: "info",
      })
    );
  }

  if (answers.location.trim()) {
    saves.push(
      api.kgSaveNode({
        folder: ["About You", "Location"],
        label: "City and timezone",
        note: answers.location.trim(),
        type: "info",
      })
    );
  }

  const detail = answers.workStudyDetail.trim();
  if (detail && answers.workStudy !== "neither") {
    if (answers.workStudy === "work" || answers.workStudy === "both") {
      saves.push(
        api.kgSaveNode({
          folder: ["Work", branchFromDetail(detail)],
          label: "Current role",
          note: detail,
          type: "info",
        })
      );
    }
    if (answers.workStudy === "study" || answers.workStudy === "both") {
      saves.push(
        api.kgSaveNode({
          folder: ["Study", branchFromDetail(detail)],
          label: "Program",
          note: detail,
          type: "info",
        })
      );
    }
  }

  await Promise.all(saves);
}

function buildWelcomeMessage(name: string): string {
  const greeting = name.trim() ? `Hi ${name.trim()}!` : "Hi there!";
  return (
    `${greeting} I'm Donna — your personal assistant.\n\n` +
    "I've saved the basics you just shared. From here you can ask me anything, tell me " +
    "what you're working on, or say **remember that…** when you want me to learn something new.\n\n" +
    "What would you like help with today?"
  );
}

interface ProfileOnboardingProps {
  onComplete: (conversationId: number) => void;
}

export default function ProfileOnboarding({ onComplete }: ProfileOnboardingProps) {
  const { config, save } = useConfig();
  const [stepIndex, setStepIndex] = useState(0);
  const [answers, setAnswers] = useState<ProfileAnswers>(() => ({
    name: "",
    age: "",
    nationality: "",
    birthday: "",
    location: detectedTimezone(),
    workStudy: "work",
    workStudyDetail: "",
  }));
  const [error, setError] = useState<string | null>(null);
  const [finishing, setFinishing] = useState(false);

  const step = STEPS[stepIndex];
  const progressSteps = STEPS.length - 1;
  const progressIndex = Math.max(0, stepIndex - 1);
  const progressPct = stepIndex === 0 ? 0 : Math.round((progressIndex / (progressSteps - 1)) * 100);

  const canContinue = useMemo(() => {
    switch (step.id) {
      case "welcome":
        return true;
      case "name":
        return answers.name.trim().length > 0;
      case "age":
        return answers.age.length > 0;
      case "nationality":
        return answers.nationality.trim().length > 0;
      case "birthday":
        return true;
      case "location":
        return answers.location.trim().length > 0;
      case "work":
        return (
          answers.workStudy === "neither" || answers.workStudyDetail.trim().length > 0
        );
      default:
        return false;
    }
  }, [step.id, answers]);

  const patch = (partial: Partial<ProfileAnswers>) => {
    setAnswers((prev) => ({ ...prev, ...partial }));
  };

  const goNext = () => {
    if (!canContinue) return;
    setError(null);
    if (stepIndex < STEPS.length - 1) {
      setStepIndex((i) => i + 1);
    } else {
      finish();
    }
  };

  const goBack = () => {
    setError(null);
    if (stepIndex > 0) setStepIndex((i) => i - 1);
  };

  const finish = async () => {
    if (!config || finishing) return;
    setFinishing(true);
    setError(null);
    try {
      await saveProfileToKnowledge(answers);
      await save({ ...config, profileOnboarded: true });

      const convId = await api.createConversation("Getting started");
      await api.addMessage(convId, "assistant", buildWelcomeMessage(answers.name));
      await api.renameConversation(convId, "Getting started");

      onComplete(convId);
    } catch (e) {
      setError(String(e));
      setFinishing(false);
    }
  };

  const inputClass =
    "w-full rounded-lg border border-white/10 bg-donna-bg px-3 py-2 text-sm text-white outline-none focus:border-donna-accent";

  return (
    <div className="flex h-full w-full items-center justify-center bg-donna-bg p-6">
      <div className="w-full max-w-lg rounded-2xl border border-white/10 bg-donna-surface p-8">
        <div className="mb-6 flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-donna-accent text-lg font-bold text-white">
            D
          </div>
          <div className="flex-1">
            <h1 className="text-xl font-semibold text-white">Getting to know you</h1>
            <p className="text-sm text-gray-400">
              A quick setup so Donna can help you properly from day one.
            </p>
          </div>
        </div>

        {stepIndex > 0 && (
          <div className="mb-6">
            <div className="mb-1 flex justify-between text-xs text-gray-500">
              <span>
                Step {progressIndex + 1} of {progressSteps}
              </span>
              <span>{progressPct}%</span>
            </div>
            <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
              <div
                className="h-full rounded-full bg-donna-accent transition-all duration-300"
                style={{ width: `${progressPct}%` }}
              />
            </div>
          </div>
        )}

        <div className="min-h-[220px] space-y-4">
          {step.id === "welcome" && (
            <div className="space-y-3 text-sm leading-relaxed text-gray-300">
              <p>
                Before your first real conversation, Donna needs a few basics — your name,
                where you are, what you do, and similar essentials.
              </p>
              <p>
                This takes about a minute. Everything is saved locally in your knowledge
                base and you can edit it anytime in the Mind Map.
              </p>
              <div className="flex items-center gap-2 rounded-lg border border-donna-accent/20 bg-donna-accent/5 px-3 py-2 text-xs text-donna-accent-light">
                <Sparkles size={14} />
                Donna won&apos;t ask about hobbies until she knows the fundamentals.
              </div>
            </div>
          )}

          {step.id === "name" && (
            <label className="block">
              <span className="mb-1 block text-sm font-medium text-white">
                What should Donna call you?
              </span>
              <input
                autoFocus
                value={answers.name}
                onChange={(e) => patch({ name: e.target.value })}
                className={inputClass}
                placeholder="e.g. David"
                onKeyDown={(e) => e.key === "Enter" && goNext()}
              />
            </label>
          )}

          {step.id === "age" && (
            <div>
              <span className="mb-2 block text-sm font-medium text-white">
                How old are you?
              </span>
              <div className="grid grid-cols-2 gap-2">
                {AGE_OPTIONS.map((opt) => {
                  const active = answers.age === opt;
                  return (
                    <button
                      key={opt}
                      type="button"
                      onClick={() => patch({ age: opt })}
                      className={`rounded-lg border px-3 py-2 text-left text-sm transition-colors ${
                        active
                          ? "border-donna-accent bg-donna-accent/15 text-donna-accent-light"
                          : "border-white/10 text-gray-300 hover:border-white/20"
                      }`}
                    >
                      {opt}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {step.id === "nationality" && (
            <label className="block">
              <span className="mb-1 block text-sm font-medium text-white">
                What nationality or country do you identify with?
              </span>
              <input
                autoFocus
                value={answers.nationality}
                onChange={(e) => patch({ nationality: e.target.value })}
                className={inputClass}
                placeholder="e.g. Vietnamese, United States"
                onKeyDown={(e) => e.key === "Enter" && goNext()}
              />
            </label>
          )}

          {step.id === "birthday" && (
            <div className="space-y-3">
              <label className="block">
                <span className="mb-1 block text-sm font-medium text-white">
                  When is your birthday?
                </span>
                <input
                  type="date"
                  value={answers.birthday}
                  onChange={(e) => patch({ birthday: e.target.value })}
                  className={inputClass}
                />
              </label>
              <button
                type="button"
                onClick={() => patch({ birthday: "" })}
                className="text-xs text-gray-500 hover:text-gray-300"
              >
                Skip for now
              </button>
            </div>
          )}

          {step.id === "location" && (
            <label className="block">
              <span className="mb-1 block text-sm font-medium text-white">
                What city or timezone are you in?
              </span>
              <input
                autoFocus
                value={answers.location}
                onChange={(e) => patch({ location: e.target.value })}
                className={inputClass}
                placeholder="e.g. Ho Chi Minh City (Asia/Ho_Chi_Minh)"
                onKeyDown={(e) => e.key === "Enter" && goNext()}
              />
              <span className="mt-1 block text-xs text-gray-500">
                Used for scheduling and timely reminders.
              </span>
            </label>
          )}

          {step.id === "work" && (
            <div className="space-y-3">
              <span className="block text-sm font-medium text-white">
                What do you do for work or study?
              </span>
              <div className="grid grid-cols-2 gap-2">
                {(
                  [
                    ["work", "I work"],
                    ["study", "I study"],
                    ["both", "Both"],
                    ["neither", "Neither right now"],
                  ] as const
                ).map(([id, label]) => (
                  <button
                    key={id}
                    type="button"
                    onClick={() => patch({ workStudy: id })}
                    className={`rounded-lg border px-3 py-2 text-left text-sm transition-colors ${
                      answers.workStudy === id
                        ? "border-donna-accent bg-donna-accent/15 text-donna-accent-light"
                        : "border-white/10 text-gray-300 hover:border-white/20"
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>
              {answers.workStudy !== "neither" && (
                <input
                  value={answers.workStudyDetail}
                  onChange={(e) => patch({ workStudyDetail: e.target.value })}
                  className={inputClass}
                  placeholder={
                    answers.workStudy === "study"
                      ? "e.g. Computer Science at MIT"
                      : "e.g. Product designer at Acme"
                  }
                  onKeyDown={(e) => e.key === "Enter" && goNext()}
                />
              )}
            </div>
          )}
        </div>

        {error && (
          <p className="mt-4 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
            {error}
          </p>
        )}

        <div className="mt-6 flex justify-between">
          <Button variant="ghost" onClick={goBack} disabled={stepIndex === 0 || finishing}>
            <ArrowLeft size={16} />
            Back
          </Button>
          <Button onClick={goNext} disabled={!canContinue || finishing}>
            {finishing ? (
              <Spinner />
            ) : stepIndex === STEPS.length - 1 ? (
              <Check size={16} />
            ) : (
              <ArrowRight size={16} />
            )}
            {stepIndex === STEPS.length - 1 ? "Start chatting" : "Continue"}
          </Button>
        </div>
      </div>
    </div>
  );
}
