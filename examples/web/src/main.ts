import { WebAudioSession, WasmEngine, type ProjectInput } from "@oxitone/web";
import type { ProjectSnapshot } from "@oxitone/protocol";
import { createSong } from "./song.js";
const status = document.querySelector<HTMLElement>("#status")!,
  play = document.querySelector<HTMLButtonElement>("#play")!;
const seek = document.querySelector<HTMLInputElement>("#seek")!,
  select = document.querySelector<HTMLSelectElement>("#song")!;
select.replaceChildren(
  ...[
    ["glass", "Glass after rain · synth + drums"],
    ["after-the-horizon", "After the horizon · full melodic dubstep"],
  ].map(([value, label]) => new Option(label!, value!)),
);
const canvas = document.querySelector<HTMLCanvasElement>("#scope")!,
  ctx = canvas.getContext("2d")!;
let session: WebAudioSession | undefined, project: ProjectInput | undefined, analyser: AnalyserNode | undefined;
let end = 0,
  running = false;
const waveform = new Float32Array(1024);
function report(error: unknown) {
  status.textContent = error instanceof Error ? error.message : String(error);
}
async function load() {
  if (!session) return;
  let candidate: ProjectInput;
  if (select.value === "glass") candidate = createSong(session.context.sampleRate);
  else {
    const response = await fetch(`/songs/${select.value}.snapshot.json`);
    if (!response.ok) throw new Error("Run pnpm example:songs first to render the full song.");
    const snapshot = (await response.json()) as ProjectSnapshot;
    snapshot.sampleRate = session.context.sampleRate;
    for (const sample of snapshot.samples) {
      const asset = await fetch(`/assets/${sample.sha256}`);
      if (!asset.ok) throw new Error("Sample asset unavailable");
      await session.importSample(new Uint8Array(await asset.arrayBuffer()), sample.format);
    }
    candidate = snapshot;
  }
  const state = await session.update(candidate);
  project = candidate;
  end = Number(state.contentEndFrame);
  seek.max = String(end);
  status.textContent = "Ready · Rust engine";
}
play.onclick = async () => {
  try {
    play.disabled = true;
    if (!session) {
      const context = new AudioContext({ latencyHint: "interactive" });
      await context.resume();
      session = await WebAudioSession.create({
        context,
        wasmUrl: "/oxitone.wasm",
        workerUrl: "/worker.js",
        workletUrl: "/worklet.js",
      });
      session.onFault = report;
      analyser = context.createAnalyser();
      analyser.fftSize = 2048;
      session.output.connect(analyser);
      await load();
      Object.assign(window, {
        oxitone: {
          session,
          analyser,
          get project() {
            return project;
          },
        },
      });
    }
    if (running) await session.pause();
    else await session.play(undefined, { startFrame: 0, endFrame: end });
    running = !running;
    play.textContent = running ? "Pause" : "Play";
  } catch (error) {
    report(error);
  } finally {
    play.disabled = false;
  }
};
document.querySelector<HTMLButtonElement>("#stop")!.onclick = () => {
  void session?.stop().then(() => {
    running = false;
    play.textContent = "Play";
  });
};
seek.onchange = () => {
  void session?.seek(Number(seek.value)).catch(report);
};
select.onchange = () => {
  void (async () => {
    if (session) {
      await session.stop();
      running = false;
      play.textContent = "Play";
      await load();
    }
  })().catch(report);
};
document.querySelector<HTMLButtonElement>("#update")!.onclick = () => {
  if (!session || select.value !== "glass") return;
  const next = createSong(session.context.sampleRate);
  next.channels[0]!.level = 0.14;
  void session
    .update(next)
    .then(() => {
      project = next;
      status.textContent = "Updated · softer glass keys";
    })
    .catch(report);
};
document.querySelector<HTMLButtonElement>("#invalid")!.onclick = () => {
  if (!session || !project) return;
  const snapshot = "snapshot" in project ? project.snapshot() : structuredClone(project);
  snapshot.channels[0]!.mixerChannelId = "mix_missing";
  void session.update(snapshot).catch(() => {
    status.textContent = "Invalid update rejected · previous song retained";
  });
};
document.querySelector<HTMLButtonElement>("#export")!.onclick = () => {
  void (async () => {
    if (!project) return;
    const e = await WasmEngine.create("/oxitone.wasm");
    try {
      const snapshot = "snapshot" in project ? project.snapshot() : project;
      for (const sample of snapshot.samples)
        e.importSample(new Uint8Array(await (await fetch(`/assets/${sample.sha256}`)).arrayBuffer()), sample.format);
      const state = e.compile(snapshot);
      const bytes = e.renderWav({
        frames: Math.min(Number(state.contentEndFrame), state.sampleRate * 8),
        bitDepth: 24,
      });
      const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "audio/wav" }));
      const link = document.createElement("a");
      link.href = url;
      link.download = "oxitone-wasm-preview.wav";
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    } finally {
      e.dispose();
    }
  })().catch(report);
};
document.addEventListener("keydown", (event) => {
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement) return;
  if (event.code === "Space") {
    event.preventDefault();
    play.click();
  }
});
setInterval(() => {
  if (!session) return;
  void session
    .state()
    .then((state) => {
      if (document.activeElement !== seek) seek.value = state.cursor;
      document.querySelector("#clock")!.textContent = (Number(state.cursor) / state.sampleRate).toFixed(1) + " s";
      const d = session!.diagnostics();
      document.querySelector("#metrics")!.textContent =
        `${d.sampleRate} Hz · ${d.bufferedFrames} buffered · ${d.underruns} underruns`;
    })
    .catch(report);
}, 250);
function draw() {
  requestAnimationFrame(draw);
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  analyser?.getFloatTimeDomainData(waveform);
  ctx.strokeStyle = "#6de7bf";
  ctx.lineWidth = 2;
  ctx.beginPath();
  for (let i = 0; i < waveform.length; i++) {
    const x = (i / waveform.length) * canvas.width,
      y = canvas.height / 2 - waveform[i]! * canvas.height * 0.8;
    if (i) ctx.lineTo(x, y);
    else ctx.moveTo(x, y);
  }
  ctx.stroke();
}
draw();
if (location.hostname === "127.0.0.1" || location.hostname === "localhost") {
  const events = new EventSource("/events");
  let revision = 0;
  events.onmessage = ({ data }) => {
    const update = JSON.parse(data) as { ok: boolean; revision: number; message: string };
    revision = update.revision;
    if (!update.ok) {
      status.textContent = `Build error · ${update.message} · previous song retained`;
      return;
    }
    if (!session || select.value !== "glass") return;
    void (async () => {
      const source = (await import(`/song.js?v=${update.revision}`)) as { createSong: typeof createSong };
      if (revision !== update.revision) return;
      const next = source.createSong(session!.context.sampleRate);
      await session!.update(next);
      project = next;
      if (revision === update.revision) status.textContent = "Code updated · Rust graph accepted";
    })().catch(report);
  };
  window.addEventListener("pagehide", () => events.close());
}
window.addEventListener("pagehide", () => {
  void session?.dispose();
  void session?.context.close();
});
