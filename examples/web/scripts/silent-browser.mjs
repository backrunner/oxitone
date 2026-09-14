import { chromium } from "playwright";

/** Test audio never opens an output device. Muting is an additional safeguard. */
export function launchSilentBrowser() {
  return chromium.launch({
    headless: true,
    ...(process.env.OXITONE_CHROME_PATH ? { executablePath: process.env.OXITONE_CHROME_PATH } : {}),
    args: ["--autoplay-policy=no-user-gesture-required", "--mute-audio"],
  });
}

export async function silentPage(browser, options = {}) {
  const page = await browser.newPage(options);
  await page.addInitScript(() => {
    const NativeContext = globalThis.AudioContext;
    const contexts = [];
    globalThis.AudioContext = class SilentAudioContext extends NativeContext {
      constructor(options = {}) {
        super({ ...options, sinkId: { type: "none" } });
        if (this.sinkId?.type !== "none") {
          void this.close();
          throw new Error("Tests require AudioContext sinkId:{type:'none'}; system output is forbidden");
        }
        contexts.push(this);
      }
      setSinkId(sink) {
        if (sink?.type !== "none") return Promise.reject(new Error("Tests cannot select a system audio output"));
        return super.setSinkId({ type: "none" });
      }
    };
    Object.defineProperty(globalThis, "oxitoneTestAudioSinks", {
      get: () =>
        contexts.map((context) => ({ sink: { type: context.sinkId?.type ?? context.sinkId }, state: context.state })),
    });
  });
  return page;
}
