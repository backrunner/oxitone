import { beatFromWire, beatToWire, ErrorCode, OxitoneError,
  type EntityId, type MarkerSpec, type ProjectSnapshot, type TimeSignatureSegment } from "@oxitone/protocol";
import { resolveBeatDuration } from "oxitone";
import { ProjectPlayback } from "./project-playback.js";
import { TempoMap, type TempoCurve, type TempoSegmentInput } from "./tempo-map.js";
import { TimeSignatureMap, type BarBeatPosition } from "./time-signature.js";

export interface Marker { id: EntityId; name?: string; startBeat: number; }

/** Authoring clocks and markers; serialization retains exact restored rational positions. */
export abstract class ProjectTimeline extends ProjectPlayback {
  private readonly tempos = new TempoMap();
  private readonly signatures = new TimeSignatureMap();
  private readonly markerList: MarkerSpec[] = [];
  abstract touch(): void;
  abstract assertMutable(): void;
  protected abstract claimId(prefix: string): EntityId;

  setTempo(bpm: number, curve?: TempoCurve): this {
    this.assertMutable();
    this.tempos.set(bpm, curve);
    this.touch();
    return this;
  }
  addTempoSegment(segment: TempoSegmentInput): this {
    this.assertMutable();
    this.tempos.add(segment);
    this.touch();
    return this;
  }
  get tempoMap(): TempoSegmentInput[] { return this.tempos.list(); }
  protected tempoSegments() { return this.tempos.toWire(); }

  setTimeSignature(numerator: number, denominator: number): this {
    this.assertMutable();
    this.signatures.set(numerator, denominator);
    this.touch();
    return this;
  }
  addTimeSignature(segment: TimeSignatureSegment): this {
    this.assertMutable();
    this.signatures.add(segment);
    this.touch();
    return this;
  }
  get timeSignatureMap(): TimeSignatureSegment[] { return this.signatures.list(); }
  barBeatToBeats(position: BarBeatPosition): number { return this.signatures.toBeats(position); }
  beatsToBarBeat(beat: number): BarBeatPosition { return this.signatures.fromBeats(beat); }
  beatsPerBarAt(bar: number): number { return this.signatures.beatsPerBarAt(bar); }
  tempoAt(beat: number): number {
    const segments = this.tempos.list();
    let bpm = segments[0]?.bpm ?? 120;
    for (const segment of segments) {
      if (segment.startBeat > beat) break;
      bpm = segment.bpm;
    }
    return bpm;
  }
  beatsForSeconds(startBeat: number, seconds: number): number {
    return resolveBeatDuration(this.snapshot(), startBeat, seconds);
  }

  addMarker(name: string, beat: number): Marker {
    this.assertMutable();
    const startBeat = beatToWire(beat);
    const marker = { id: this.claimId("mrk_"), name, startBeat };
    this.markerList.push(marker);
    this.touch();
    return { ...marker, startBeat: beat };
  }
  get markers(): readonly Marker[] {
    return this.markerList.map((marker) => ({ id: marker.id, startBeat: beatFromWire(marker.startBeat),
      ...(marker.name === undefined ? {} : { name: marker.name }) }));
  }
  /** @internal Detached exact wire markers for serialization. */
  markerSpecs(): MarkerSpec[] { return structuredClone(this.markerList); }

  protected restoreTimeline(snapshot: ProjectSnapshot): void {
    this.tempos.restore(snapshot.tempoMap);
    const first = snapshot.timeSignatureMap[0]!;
    if (first.startBar !== 1) throw new OxitoneError(ErrorCode.InvalidProject, "time signature map must start at bar 1");
    this.signatures.set(first.numerator, first.denominator);
    for (const segment of snapshot.timeSignatureMap.slice(1)) this.signatures.add(segment);
    this.markerList.push(...structuredClone(snapshot.markers));
  }
}
