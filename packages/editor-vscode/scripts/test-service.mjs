import { ProjectDocument } from "../../cli/dist/source/project-document.js";
import { DocumentDispatcher } from "../../cli/dist/source/document-dispatch.js";
import { openDocumentBridge } from "../../cli/dist/source/document-bridge.js";

const document = await ProjectDocument.open({ entry: process.argv[2] });
if (document.view.status !== "ready") throw new Error(JSON.stringify(document.view.diagnostic));
const dispatcher = new DocumentDispatcher(document);
const stop = await openDocumentBridge(process.argv[3], document, dispatcher);
process.stdout.write("READY\n");
process.once("SIGTERM", async () => {
  await stop();
  dispatcher.close();
  document.close();
});
