/** Interactive question Donna embeds in her replies via ```donna-ask blocks. */
export interface DonnaQuestion {
  type: "choice" | "text";
  prompt: string;
  options?: string[];
}

export type MessageSegment =
  | { kind: "markdown"; text: string }
  | { kind: "question"; question: DonnaQuestion };

const BLOCK_RE = /```donna-ask\n([\s\S]*?)```/g;

function parseQuestionJson(raw: string): DonnaQuestion | null {
  try {
    const v = JSON.parse(raw.trim()) as Record<string, unknown>;
    const prompt = typeof v.prompt === "string" ? v.prompt.trim() : "";
    if (!prompt) return null;

    const type = v.type === "choice" ? "choice" : v.type === "text" ? "text" : null;
    if (!type) return null;

    if (type === "choice") {
      const opts = Array.isArray(v.options)
        ? v.options.filter((o): o is string => typeof o === "string" && o.trim().length > 0)
        : [];
      if (opts.length < 2) return null;
      const hasOther = opts.some((o) => o.toLowerCase() === "other");
      const options = hasOther ? opts : [...opts, "Other"];
      return { type: "choice", prompt, options };
    }

    return { type: "text", prompt };
  } catch {
    return null;
  }
}

/** Split assistant message content into markdown segments and question widgets. */
export function parseDonnaMessage(content: string): MessageSegment[] {
  const segments: MessageSegment[] = [];
  let lastIndex = 0;

  for (const match of content.matchAll(BLOCK_RE)) {
    const start = match.index ?? 0;
    const before = content.slice(lastIndex, start);
    if (before.trim()) segments.push({ kind: "markdown", text: before });

    const q = parseQuestionJson(match[1] ?? "");
    if (q) segments.push({ kind: "question", question: q });

    lastIndex = start + match[0].length;
  }

  const tail = content.slice(lastIndex);
  if (tail.trim()) segments.push({ kind: "markdown", text: tail });

  if (segments.length === 0 && content.trim()) {
    segments.push({ kind: "markdown", text: content });
  }

  return segments;
}

/** True when content has at least one complete, parseable question block. */
export function hasDonnaQuestions(content: string): boolean {
  return parseDonnaMessage(content).some((s) => s.kind === "question");
}

export function questionSegments(content: string): DonnaQuestion[] {
  return parseDonnaMessage(content)
    .filter((s): s is Extract<MessageSegment, { kind: "question" }> => s.kind === "question")
    .map((s) => s.question);
}

/** Format multiple answers as a numbered list for Donna. */
export function formatNumberedAnswers(answers: string[]): string {
  return answers.map((a, i) => `${i + 1}. ${a.trim()}`).join("\n");
}

const DONNA_ASK_OPEN = "```donna-ask";

/** Strip a trailing incomplete markdown code fence so raw ``` never flashes during streaming. */
function stripTrailingIncompleteFence(text: string): string {
  const last = text.lastIndexOf("```");
  if (last === -1) return text;

  const tail = text.slice(last);
  if (tail === "```" || tail === "``" || tail === "`") {
    return text.slice(0, last);
  }

  // Opening fence without a closing line yet (e.g. ```donna-ask or ```json).
  const hasClosingLine = /\n```\s*$/.test(tail);
  if (!hasClosingLine && tail.startsWith("```")) {
    return text.slice(0, last);
  }

  return text;
}

/**
 * While the assistant reply is streaming, hide incomplete donna-ask blocks and surface
 * only markdown + fully parsed questions.
 */
export function parseDonnaMessageStreaming(content: string): {
  segments: MessageSegment[];
  pendingQuestion: boolean;
} {
  const lastOpen = content.lastIndexOf(DONNA_ASK_OPEN);
  if (lastOpen === -1) {
    const visible = stripTrailingIncompleteFence(content);
    return {
      segments: visible.trim() ? parseDonnaMessage(visible) : [],
      pendingQuestion: false,
    };
  }

  const afterOpen = content.slice(lastOpen);
  const completeBlock = /^```donna-ask\n[\s\S]*?\n```/.test(afterOpen);

  if (completeBlock) {
    return {
      segments: parseDonnaMessage(content),
      pendingQuestion: false,
    };
  }

  const visible = content.slice(0, lastOpen);
  return {
    segments: visible.trim() ? parseDonnaMessage(visible) : [],
    pendingQuestion: true,
  };
}
