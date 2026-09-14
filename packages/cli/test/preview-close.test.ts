import { createConnection, createServer, type Socket } from "node:net";
import { once } from "node:events";
import { expect, it } from "vitest";
import { PreviewConnection } from "../src/preview/launch.js";

it("stops queued queries when the viewer half-closes its socket", async () => {
  const server = createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("missing socket address");
  const accepted = once(server, "connection");
  const client = createConnection(address.port, "127.0.0.1");
  const [viewer] = (await accepted) as [Socket];
  const errors: Error[] = [];
  client.on("error", (error) => errors.push(error));
  viewer.on("error", (error) => errors.push(error));
  const connection = new PreviewConnection(client);
  try {
    connection.send({ protocolVersion: "1.0", type: "query" });
    await once(viewer, "data");
    connection.send({ protocolVersion: "1.0", type: "query" });
    const ended = once(client, "end");
    viewer.end();
    await ended;
    const before = client.bytesWritten;
    for (let i = 0; i < 100; i++) connection.send({ protocolVersion: "1.0", type: "query" });
    await new Promise((resolve) => setImmediate(resolve));
    expect(client.bytesWritten).toBe(before);
    expect(errors).toEqual([]);
  } finally {
    connection.close();
    client.destroy();
    viewer.destroy();
    await new Promise<void>((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())));
  }
});
