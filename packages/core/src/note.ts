import { ErrorCode, OxitoneError, type Beat, type Pitch } from "@oxitone/protocol";

/** Authoring input for a single note (fields per `NoteSpec`). */
export interface NoteInput {
  pitch: Pitch;
  start: Beat;
  duration: Beat;
  velocity: number;
  offVelocity?: number;
  chance?: number;
  voice?: number;
  tags?: readonly string[];
}

function checkRange(
  value: number,
  min: number,
  max: number,
  path: string,
  integer = false,
): void {
  const valid =
    Number.isFinite(value) && value >= min && value <= max && (!integer || Number.isInteger(value));
  if (!valid) {
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      `note ${path} must be ${integer ? "an integer " : ""}in ${min}..${max}, got ${value}`,
      { details: { path: `note.${path}` } },
    );
  }
}

/** Validate a note against the `NoteSpec` field ranges. */
export function validateNote(note: NoteInput): void {
  checkRange(note.pitch, 0, 127, "pitch", true);
  if (!Number.isFinite(note.start) || note.start < 0) {
    throw new OxitoneError(ErrorCode.InvalidProject, `note start must be >= 0, got ${note.start}`, {
      details: { path: "note.start" },
    });
  }
  if (!Number.isFinite(note.duration) || note.duration <= 0) {
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      `note duration must be > 0, got ${note.duration}`,
      { details: { path: "note.duration" } },
    );
  }
  checkRange(note.velocity, 0, 1, "velocity");
  if (note.offVelocity !== undefined) {
    checkRange(note.offVelocity, 0, 1, "offVelocity");
  }
  if (note.chance !== undefined) {
    checkRange(note.chance, 0, 1, "chance");
  }
  if (note.voice !== undefined) {
    checkRange(note.voice, 0, Number.MAX_SAFE_INTEGER, "voice", true);
  }
}

/** Deep-enough copy of a note (tags array is duplicated), then frozen. */
export function freezeNote(note: NoteInput): Readonly<NoteInput> {
  validateNote(note);
  const copy: NoteInput = {
    pitch: note.pitch,
    start: note.start,
    duration: note.duration,
    velocity: note.velocity,
  };
  if (note.offVelocity !== undefined) {
    copy.offVelocity = note.offVelocity;
  }
  if (note.chance !== undefined) {
    copy.chance = note.chance;
  }
  if (note.voice !== undefined) {
    copy.voice = note.voice;
  }
  if (note.tags !== undefined) {
    copy.tags = Object.freeze([...note.tags]);
  }
  return Object.freeze(copy);
}

/**
 * Stable ordering for simultaneous notes: `start`, then voice hint, then
 * original creation order.
 */
export function sortNotes(notes: readonly NoteInput[]): NoteInput[] {
  return notes
    .map((note, index) => ({ note, index }))
    .sort((a, b) => {
      const byStart = a.note.start - b.note.start;
      if (byStart !== 0) {
        return byStart;
      }
      const byVoice = (a.note.voice ?? 0) - (b.note.voice ?? 0);
      if (byVoice !== 0) {
        return byVoice;
      }
      return a.index - b.index;
    })
    .map(({ note }) => note);
}
