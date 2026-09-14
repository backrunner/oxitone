import type { SourceNote } from "@oxitone/protocol";
import type { NoteInput } from "../notes/note.js";
import type { ReadonlySourceNote } from "./types.js";

/** Convert schema optional values to the exact authoring shape. */
export function sourceNoteInput(note: ReadonlySourceNote): NoteInput {
  return { pitch: note.pitch, start: note.start, duration: note.duration, velocity: note.velocity,
    ...(note.offVelocity === undefined ? {} : { offVelocity: note.offVelocity }),
    ...(note.chance === undefined ? {} : { chance: note.chance }),
    ...(note.voice === undefined ? {} : { voice: note.voice }),
    ...(note.tags === undefined ? {} : { tags: note.tags }),
  };
}

export function noteSourceInput(note: Readonly<NoteInput>): SourceNote {
  return { pitch: note.pitch, start: note.start, duration: note.duration, velocity: note.velocity,
    ...(note.offVelocity === undefined ? {} : { offVelocity: note.offVelocity }),
    ...(note.chance === undefined ? {} : { chance: note.chance }),
    ...(note.voice === undefined ? {} : { voice: note.voice }),
    ...(note.tags === undefined ? {} : { tags: [...note.tags] }),
  };
}
