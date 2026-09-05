import { ID_PREFIXES, Pcg32, type EntityId } from "@oxitone/protocol";

export { ID_PREFIXES };

/**
 * Deterministic entity ID generator. The same seed always produces the same
 * ID sequence, which keeps snapshots reproducible in tests; inject a seed via
 * the constructor (or `Project` options) to control it.
 */
export class IdGenerator {
  private readonly rng: Pcg32;
  private counter = 0;

  constructor(seed: number | bigint = 0) {
    this.rng = new Pcg32(seed);
  }

  /** Generate the next ID for the given prefix (e.g. `ID_PREFIXES.track`). */
  next(prefix: string): EntityId {
    this.counter += 1;
    const serial = this.counter.toString(36);
    const random = this.rng.nextU32().toString(36);
    return `${prefix}${serial}${random}`;
  }
}

/**
 * Shared generator for standalone entities (patterns from `chord`/`arp`)
 * created outside a project. Deterministic per process; pass an explicit
 * `id` when an ID must survive across runs.
 */
export const defaultIdGenerator: IdGenerator = new IdGenerator(0x4f58_4954);
