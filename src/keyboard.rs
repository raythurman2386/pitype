//! A small on-screen keyboard model: which key a character lives on and
//! whether reaching it needs Shift. Pure data so the UI can highlight keys
//! without the typing engine knowing anything about keyboards.

/// One key in a layout row. `width` is in key units (1.0 = a letter key).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyDef {
    /// Primary legend shown on the cap, lowercase.
    pub label: &'static str,
    /// Key width in units; 1.0 for letters, more for space/modifiers.
    pub width: f32,
    /// Modifier-style keys get a dimmer cap.
    pub modifier: bool,
    /// Icon asset path (e.g. "icons/delete.svg") rendered instead of the
    /// text label; `None` for plain character keys.
    pub icon: Option<&'static str>,
}

const fn key(label: &'static str) -> KeyDef {
    KeyDef {
        label,
        width: 1.0,
        modifier: false,
        icon: None,
    }
}

const fn modifier(label: &'static str, width: f32) -> KeyDef {
    KeyDef {
        label,
        width,
        modifier: true,
        icon: None,
    }
}

const fn modifier_icon(label: &'static str, width: f32, icon: &'static str) -> KeyDef {
    KeyDef {
        label,
        width,
        modifier: true,
        icon: Some(icon),
    }
}

/// A physical ANSI keyboard, minus keys the lessons never ask for
/// (arrows, function rows, number pad). Row 0 is the number row.
pub const ROWS: &[&[KeyDef]] = &[
    &[
        key("`"),
        key("1"),
        key("2"),
        key("3"),
        key("4"),
        key("5"),
        key("6"),
        key("7"),
        key("8"),
        key("9"),
        key("0"),
        key("-"),
        key("="),
        modifier_icon("backspace", 2.0, "icons/delete.svg"),
    ],
    &[
        modifier_icon("tab", 1.5, "icons/arrow-right-to-line.svg"),
        key("q"),
        key("w"),
        key("e"),
        key("r"),
        key("t"),
        key("y"),
        key("u"),
        key("i"),
        key("o"),
        key("p"),
        key("["),
        key("]"),
        modifier("\\", 1.5),
    ],
    &[
        modifier_icon("caps", 1.75, "icons/case-upper.svg"),
        key("a"),
        key("s"),
        key("d"),
        key("f"),
        key("g"),
        key("h"),
        key("j"),
        key("k"),
        key("l"),
        key(";"),
        key("'"),
        modifier_icon("enter", 2.25, "icons/corner-down-left.svg"),
    ],
    &[
        modifier_icon("shift", 2.25, "icons/arrow-big-up.svg"),
        key("z"),
        key("x"),
        key("c"),
        key("v"),
        key("b"),
        key("n"),
        key("m"),
        key(","),
        key("."),
        key("/"),
        modifier_icon("shift", 2.75, "icons/arrow-big-up.svg"),
    ],
    &[
        modifier("ctrl", 1.5),
        modifier("alt", 1.5),
        modifier("space", 7.0),
        modifier("alt", 1.5),
        modifier("ctrl", 1.5),
    ],
];

/// Total width of the widest row, in key units.
pub const ROW_WIDTH: f32 = 15.0;

/// Where a character lives on the layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyHit {
    /// Row index into `ROWS`, then index within the row.
    pub row: usize,
    pub index: usize,
    /// The key needs Shift held to produce this character.
    pub shift: bool,
}

/// Map a character to its key on the US layout, if it has one.
/// Uppercase letters map to the letter key with `shift: true`; shifted
/// symbols (`!`, `_`, `?`, …) map to their base key with `shift: true`.
pub fn key_for_char(c: char) -> Option<KeyHit> {
    let (ch, shift) = match c {
        'A'..='Z' => (c.to_ascii_lowercase(), true),
        _ => (c, false),
    };
    let (label, shift) = match ch {
        ' ' => ("space", shift),
        '!' => ("1", true),
        '@' => ("2", true),
        '#' => ("3", true),
        '$' => ("4", true),
        '%' => ("5", true),
        '^' => ("6", true),
        '&' => ("7", true),
        '*' => ("8", true),
        '(' => ("9", true),
        ')' => ("0", true),
        '_' => ("-", true),
        '+' => ("=", true),
        '{' => ("[", true),
        '}' => ("]", true),
        '|' => ("\\", true),
        ':' => (";", true),
        '"' => ("'", true),
        '<' => (",", true),
        '>' => (".", true),
        '?' => ("/", true),
        '~' => ("`", true),
        _ => (single_char_label(ch)?, shift),
    };
    key_for_label_with_shift(label, shift)
}

/// Look up a key by its layout label (e.g. "backspace", "enter", "a").
pub fn key_for_label(label: &str) -> Option<KeyHit> {
    key_for_label_with_shift(label, false)
}

fn key_for_label_with_shift(label: &str, shift: bool) -> Option<KeyHit> {
    ROWS.iter().enumerate().find_map(|(row, keys)| {
        keys.iter()
            .enumerate()
            .find_map(|(index, key)| (key.label == label).then_some(KeyHit { row, index, shift }))
    })
}

/// Allocation-free label for the printable ASCII characters that need no
/// shift mapping; `None` for anything without a key.
fn single_char_label(ch: char) -> Option<&'static str> {
    // One label per key in `ROWS` order; a byte match gives the slice bounds.
    const SINGLES: &str = "`1234567890-=qwertyuiop[]\\asdfghjkl;'zxcvbnm,./";
    let byte = ch.to_ascii_lowercase();
    if !byte.is_ascii() {
        return None;
    }
    let pos = SINGLES.as_bytes().iter().position(|&k| k == byte as u8)?;
    Some(&SINGLES[pos..pos + 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(c: char) -> KeyHit {
        key_for_char(c).unwrap()
    }

    #[test]
    fn letters_map_without_shift() {
        let hit = hit('t');
        assert_eq!(ROWS[hit.row][hit.index].label, "t");
        assert!(!hit.shift);
    }

    #[test]
    fn uppercase_maps_with_shift() {
        let hit = hit('T');
        assert_eq!(ROWS[hit.row][hit.index].label, "t");
        assert!(hit.shift);
    }

    #[test]
    fn shifted_symbols_map_with_shift() {
        let bang = hit('!');
        assert_eq!(ROWS[bang.row][bang.index].label, "1");
        assert!(bang.shift);
        let dash = hit('_');
        assert_eq!(ROWS[dash.row][dash.index].label, "-");
        assert!(dash.shift);
    }

    #[test]
    fn unshifted_punct_maps_plain() {
        let semi = hit(';');
        assert_eq!(ROWS[semi.row][semi.index].label, ";");
        assert!(!semi.shift);
    }

    #[test]
    fn space_maps_to_spacebar() {
        let hit = hit(' ');
        assert_eq!(ROWS[hit.row][hit.index].label, "space");
        assert!(!hit.shift);
    }

    #[test]
    fn every_lesson_char_resolves() {
        // The whole lesson alphabet: words, gentle punctuation, sentence
        // stops, hard words, and capitalization.
        let text =
            "The quick brown fox! Well-known, don't; rhythm? Sphinx of black quartz: judge my vow.";
        for c in text.chars() {
            assert!(
                key_for_char(c).is_some(),
                "lesson char {c:?} has no key mapping"
            );
        }
    }

    #[test]
    fn unmappable_chars_return_none() {
        assert!(key_for_char('\n').is_none());
        assert!(key_for_char('é').is_none());
    }

    #[test]
    fn label_lookup_finds_modifiers() {
        let backspace = key_for_label("backspace").unwrap();
        assert_eq!(ROWS[backspace.row][backspace.index].label, "backspace");
        assert!(!backspace.shift);
        let space = key_for_label("space").unwrap();
        assert_eq!(ROWS[space.row][space.index].label, "space");
        assert!(key_for_label("no-such-key").is_none());
    }

    #[test]
    fn rows_fit_within_row_width() {
        // The renderer staggers rows by half a unit; every row must fit the
        // declared width so staggering stays aligned.
        for row in ROWS {
            let width: f32 = row.iter().map(|k| k.width).sum();
            assert!(width <= ROW_WIDTH, "row width {width} exceeds {ROW_WIDTH}");
        }
        // The widest row (row 0) defines the board width exactly.
        let widest: f32 = ROWS[0].iter().map(|k| k.width).sum();
        assert_eq!(widest, ROW_WIDTH);
    }
}
