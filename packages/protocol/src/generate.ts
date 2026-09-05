/**
 * Regenerate everything under `schemas/`: JSON Schemas and the canonical
 * protocol fixtures. Run via `pnpm schemas` from the repository root.
 */
import { generateSchemas } from "./gen/schemas.js";
import { generateSnapshotFixture } from "./gen/snapshot.js";
import {
  generateAutomationFixtures,
  generateHash64Vectors,
  generatePcg32Vectors,
} from "./gen/vectors.js";

generateSchemas();
generateSnapshotFixture();
generateAutomationFixtures();
generatePcg32Vectors();
generateHash64Vectors();
