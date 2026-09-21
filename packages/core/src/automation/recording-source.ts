import { AutomationSource } from "./source.js";
import type { RecordedParameterSpan } from "@oxitone/protocol";

/** A host-applied constant value on a half-open interval in Project beats. */
export type Vst3RecordedSpan = Readonly<RecordedParameterSpan>;
function lowerBound(spans: readonly Vst3RecordedSpan[], test: (span: Vst3RecordedSpan) => boolean): number {
  let lo = 0,
    hi = spans.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (test(spans[mid]!)) hi = mid;
    else lo = mid + 1;
  }
  return lo;
}
/** Later recorded passes replace earlier overlaps; preserve the base generator outside the take. */
export function recordedRanges(spans: readonly Vst3RecordedSpan[], parameterId: number) {
  const timeline: Vst3RecordedSpan[] = [];
  for (const span of spans) {
    if (span.parameterId !== parameterId) continue;
    const first = lowerBound(timeline, (old) => old.end > span.start);
    const last = lowerBound(timeline, (old) => old.start >= span.end);
    const left = timeline[first],
      right = timeline[last - 1];
    timeline.splice(
      first,
      last - first,
      ...(left && left.start < span.start ? [{ ...left, end: span.start }] : []),
      span,
      ...(right && right.end > span.end ? [{ ...right, start: span.end }] : []),
    );
  }
  const ranges = [];
  for (let index = 0; index < timeline.length;) {
    const first = timeline[index++]!;
    let end = first.end,
      value = first.value;
    const points = [{ beat: 0, value, curve: { kind: "step" as const } }];
    while (index < timeline.length && timeline[index]!.start === end) {
      const next = timeline[index++]!;
      if (next.value !== value)
        points.push({ beat: next.start - first.start, value: next.value, curve: { kind: "step" } });
      value = next.value;
      end = next.end;
    }
    ranges.push({ start: first.start, end, points });
  }
  return ranges;
}
export function recordedSource(
  spans: readonly Vst3RecordedSpan[],
  parameterId: number,
  base: AutomationSource,
): AutomationSource {
  return recordedRanges(spans, parameterId).reduce(
    (source, range) => source.replaceRange({ start: range.start, end: range.end }, range.points),
    base,
  );
}
