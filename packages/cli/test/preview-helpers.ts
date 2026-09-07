import { connect, type Socket } from "node:net";
import { setTimeout as delay } from "node:timers/promises";
import { encodeFrame, FrameDecoder } from "../src/preview/framing.js";

export async function until(check: () => boolean | Promise<boolean>, timeout = 30_000): Promise<void> {
  const end = Date.now() + timeout;
  while (!(await check())) { if (Date.now() > end) throw new Error("Preview condition timed out"); await delay(50); }
}

export class Client {
  private replies: { resolve: (value: Record<string, unknown>) => void; reject: (error: Error) => void }[] = [];
  constructor(readonly socket: Socket) {
    const decoder = new FrameDecoder();
    socket.on("data", (chunk: Buffer) => {
      try { for (const value of decoder.push(chunk)) this.replies.shift()?.resolve(value as Record<string, unknown>); }
      catch (error) { socket.destroy(error as Error); }
    });
    socket.on("error", (error) => { for (const reply of this.replies.splice(0)) reply.reject(error); });
    socket.on("close", () => { for (const reply of this.replies.splice(0)) reply.reject(new Error("Preview disconnected")); });
  }
  request(frame: unknown): Promise<Record<string, unknown>> {
    return new Promise((resolve, reject) => {
      // A rejected request invalidates FIFO matching; close before a late reply can
      // be mistaken for a subsequent query. Native cold compilation has its own budget.
      const timer = setTimeout(() => this.socket.destroy(new Error(`Preview response timed out: ${JSON.stringify(frame).slice(0,120)}`)), 30_000);
      this.replies.push({ resolve: (value) => { clearTimeout(timer); resolve(value); }, reject: (error) => { clearTimeout(timer); reject(error); } });
      this.socket.write(encodeFrame(frame));
    });
  }
  query(): Promise<Record<string, unknown>> { return this.request({ protocolVersion: "1.0", type: "query" }); }
  static async open(path: string): Promise<Client> {
    let socket: Socket | undefined;
    await until(async () => {
      try {
        socket = await new Promise<Socket>((resolve, reject) => {
          const candidate = connect(path);
          candidate.once("error", (error) => { candidate.destroy(); reject(error); });
          candidate.once("connect", () => { candidate.removeAllListeners("error"); resolve(candidate); });
        });
        return true;
      } catch { return false; }
    });
    return new Client(socket!);
  }
}
