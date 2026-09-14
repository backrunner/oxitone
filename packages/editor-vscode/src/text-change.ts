/** One UTF-16 replacement; keep common text and never split a surrogate pair or CRLF. */
export function textChange(before: string, after: string): { start: number; end: number; text: string } {
  let start = 0,
    end = before.length,
    nextEnd = after.length;
  while (start < end && start < nextEnd && before[start] === after[start]) start++;
  const splitsPair = (text: string, index: number) =>
    index > 0 &&
    index < text.length &&
    ((text.charCodeAt(index - 1) >= 0xd800 &&
      text.charCodeAt(index - 1) <= 0xdbff &&
      text.charCodeAt(index) >= 0xdc00 &&
      text.charCodeAt(index) <= 0xdfff) ||
      (text[index - 1] === "\r" && text[index] === "\n"));
  if (splitsPair(before, start) || splitsPair(after, start)) start--;
  while (end > start && nextEnd > start && before[end - 1] === after[nextEnd - 1]) {
    end--;
    nextEnd--;
  }
  if (splitsPair(before, end) || splitsPair(after, nextEnd)) {
    end++;
    nextEnd++;
  }
  return { start, end, text: after.slice(start, nextEnd) };
}
