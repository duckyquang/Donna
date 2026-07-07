// Desktop voice: push-to-talk mic capture + transcription + spoken replies.
// Mirrors server.ts's bearer-header convention for the two voice endpoints
// donna-server exposes (POST /voice/transcribe, POST /voice/speak).

import { serverConfig } from "./server";

export interface Recording {
  stop: () => Promise<Blob>;
}

/** Start recording the mic. Throws a clear error if permission is denied. */
export async function recordAudio(): Promise<Recording> {
  let stream: MediaStream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  } catch {
    throw new Error("Microphone access denied — enable it in System Settings.");
  }

  const mimeType =
    typeof MediaRecorder.isTypeSupported === "function" &&
    MediaRecorder.isTypeSupported("audio/webm")
      ? "audio/webm"
      : undefined;
  const recorder = new MediaRecorder(stream, mimeType ? { mimeType } : undefined);
  const chunks: Blob[] = [];
  recorder.ondataavailable = (e) => {
    if (e.data.size > 0) chunks.push(e.data);
  };
  recorder.start();

  return {
    stop: () =>
      new Promise<Blob>((resolve) => {
        recorder.onstop = () => {
          stream.getTracks().forEach((t) => t.stop());
          resolve(new Blob(chunks, { type: mimeType ?? "audio/webm" }));
        };
        recorder.stop();
      }),
  };
}

/** Send a recorded clip to donna-server and return the transcript. */
export async function transcribeBlob(blob: Blob): Promise<string> {
  const { url, token } = serverConfig();
  const form = new FormData();
  form.append("file", blob, "clip.webm");
  const res = await fetch(`${url}/voice/transcribe`, {
    method: "POST",
    headers: { authorization: `Bearer ${token}` },
    body: form,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(body.error ?? `transcribe failed (${res.status})`);
  }
  const { transcript } = await res.json();
  return transcript;
}

/** Speak text aloud via donna-server TTS. Best-effort — errors are logged, not thrown. */
export async function speak(text: string): Promise<void> {
  try {
    const { url, token } = serverConfig();
    const res = await fetch(`${url}/voice/speak`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ text }),
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({ error: res.statusText }));
      throw new Error(body.error ?? `speak failed (${res.status})`);
    }
    const blob = await res.blob();
    const url2 = URL.createObjectURL(blob);
    const audio = new Audio(url2);
    audio.onended = () => URL.revokeObjectURL(url2);
    await audio.play();
  } catch (e) {
    console.error("voice.speak failed:", e);
  }
}
