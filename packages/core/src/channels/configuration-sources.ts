import type { EffectRef, InstrumentRef } from "@oxitone/protocol";

/** Control-side source identity only; never serialized into music or used for parameter binding. */
export class ConfigurationSources {
  instrument: InstrumentRef | undefined;
  chain: readonly EffectRef[] = [];
  constructor(instrument: InstrumentRef | undefined, chain: readonly EffectRef[]) {
    this.instrument = instrument; this.chain = chain;
  }
  update(patch: { instrument?: InstrumentRef; effectChain?: readonly EffectRef[]; inserts?: readonly EffectRef[] }): void {
    if (patch.instrument) this.instrument = patch.instrument;
    if (patch.effectChain) this.chain = patch.effectChain;
    if (patch.inserts) this.chain = patch.inserts;
  }
}
