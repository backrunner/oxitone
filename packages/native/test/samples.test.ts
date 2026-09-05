import { describe, expect, it } from "vitest";
import { ErrorCode } from "@oxitone/protocol";
import { loadNativeBinding } from "../src/load.js";

describe("versioned sample inspection command", () => {
  it("returns structured errors for malformed commands and checks version before file access", () => {
    const binding = loadNativeBinding();
    for (const [request, code] of [
      ["not json", ErrorCode.InvalidProject],
      [JSON.stringify({ protocolVersion: "1.0" }), ErrorCode.InvalidProject],
      [JSON.stringify({ protocolVersion: "99.0", path: "" }), ErrorCode.ProtocolVersionUnsupported],
    ]) {
      try {
        binding.inspectSample(request!);
        expect.unreachable("invalid inspection request succeeded");
      } catch (error) {
        expect(JSON.parse((error as Error).message)).toMatchObject({ code });
      }
    }
  });
});
