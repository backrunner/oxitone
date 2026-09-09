//! Presentation-only fallback headings for descriptor parameter namespaces.
pub fn title(namespace: &str) -> String {
    namespace
        .split(['.', '_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut output = String::new();
            let chars: Vec<_> = part.chars().collect();
            for (index, ch) in chars.iter().enumerate() {
                if index > 0
                    && ch.is_uppercase()
                    && (chars[index - 1].is_lowercase()
                        || chars.get(index + 1).is_some_and(|next| next.is_lowercase()))
                {
                    output.push(' ');
                }
                if index == 0 {
                    output.extend(ch.to_uppercase());
                } else {
                    output.push(*ch);
                }
            }
            output
        })
        .collect::<Vec<_>>()
        .join(" / ")
}
