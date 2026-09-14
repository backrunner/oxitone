import { createHash } from "node:crypto";
import {
  canonicalEncode,
  ErrorCode,
  OxitoneError,
  type DocumentMessage,
  type DocumentRequest,
} from "@oxitone/protocol";

type Response = Extract<DocumentMessage, { type: "response" }>;
type Entry = { fingerprint: string; result: Promise<Response> };

/** Monotonic client streams bound replay protection by clients, rather than session duration. */
export class DocumentRequestHistory {
  private readonly results = new Map<string, Entry>();
  private readonly streams = new Map<string, number>();
  fingerprint(request: DocumentRequest): string {
    return createHash("sha256").update(canonicalEncode(request)).digest("hex");
  }
  get(id: string): Entry | undefined {
    return this.results.get(id);
  }
  private stream(id: string): { client: string; sequence: number } {
    if (!id.startsWith("stream/"))
      throw new OxitoneError(ErrorCode.SourceChanged, "request IDs must use a monotonic client stream");
    const match = /^stream\/([A-Za-z0-9_-]{1,64})\/([1-9][0-9]*)$/.exec(id),
      sequence = Number(match?.[2]);
    if (!match || !Number.isSafeInteger(sequence))
      throw new OxitoneError(ErrorCode.SourceChanged, "invalid request stream sequence");
    return { client: match[1]!, sequence };
  }
  reserve(id: string): void {
    const stream = this.stream(id);
    const previous = this.streams.get(stream.client);
    if (previous !== undefined && stream.sequence <= previous)
      throw new OxitoneError(ErrorCode.SourceChanged, "request result expired; refresh before retrying");
    if (previous === undefined && this.streams.size >= 64)
      throw new OxitoneError(ErrorCode.BudgetExceeded, "document client stream budget exceeded");
    this.streams.set(stream.client, stream.sequence);
  }
  remember(id: string, entry: Entry): void {
    this.results.set(id, entry);
  }
  settle(): void {
    while (this.results.size > 256) {
      const id = this.results.keys().next().value!;
      this.results.delete(id);
    }
  }
}
