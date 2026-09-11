//! Word pools, quotes, and prompt generation for typing lessons.

use std::time::Duration;

/// Lesson selection: a timed run, a fixed word count, or a specific quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptSpec {
    Time(Duration),
    Words(usize),
    Quote(usize),
}

impl PromptSpec {
    /// Stable identifier used for stats keys and the settings file.
    pub fn mode_tag(&self) -> String {
        match self {
            PromptSpec::Time(d) => format!("time-{}", d.as_secs()),
            PromptSpec::Words(n) => format!("words-{n}"),
            PromptSpec::Quote(i) => format!("quote-{i}"),
        }
    }

    /// The durations offered on the menu.
    pub const TIME_CHOICES: [u64; 4] = [15, 30, 60, 120];
    /// The word counts offered on the menu.
    pub const WORD_CHOICES: [usize; 4] = [10, 25, 50, 100];
    pub const QUOTE_COUNT: usize = 12;
}

const COMMON_WORDS: &[&str] = &[
    "the",
    "of",
    "and",
    "to",
    "in",
    "is",
    "you",
    "that",
    "it",
    "he",
    "was",
    "for",
    "on",
    "are",
    "as",
    "with",
    "his",
    "they",
    "at",
    "be",
    "this",
    "have",
    "from",
    "or",
    "one",
    "had",
    "by",
    "word",
    "but",
    "not",
    "what",
    "all",
    "were",
    "we",
    "when",
    "your",
    "can",
    "said",
    "there",
    "use",
    "an",
    "each",
    "which",
    "she",
    "do",
    "how",
    "their",
    "if",
    "will",
    "up",
    "other",
    "about",
    "out",
    "many",
    "then",
    "them",
    "these",
    "so",
    "some",
    "her",
    "would",
    "make",
    "like",
    "him",
    "into",
    "time",
    "has",
    "look",
    "two",
    "more",
    "write",
    "go",
    "see",
    "number",
    "no",
    "way",
    "could",
    "people",
    "my",
    "than",
    "first",
    "water",
    "been",
    "call",
    "who",
    "oil",
    "its",
    "now",
    "find",
    "long",
    "down",
    "day",
    "did",
    "get",
    "come",
    "made",
    "may",
    "part",
    "over",
    "new",
    "sound",
    "take",
    "only",
    "little",
    "work",
    "know",
    "place",
    "year",
    "live",
    "me",
    "back",
    "give",
    "most",
    "very",
    "after",
    "thing",
    "our",
    "just",
    "name",
    "good",
    "sentence",
    "man",
    "think",
    "say",
    "great",
    "where",
    "help",
    "through",
    "much",
    "before",
    "line",
    "right",
    "too",
    "mean",
    "old",
    "any",
    "same",
    "tell",
    "boy",
    "follow",
    "came",
    "want",
    "show",
    "also",
    "around",
    "form",
    "three",
    "small",
    "set",
    "put",
    "end",
    "does",
    "another",
    "well",
    "large",
    "must",
    "big",
    "even",
    "such",
    "because",
    "turn",
    "here",
    "why",
    "ask",
    "went",
    "men",
    "read",
    "need",
    "land",
    "different",
    "home",
    "us",
    "move",
    "try",
    "kind",
    "hand",
    "picture",
    "again",
    "change",
    "off",
    "play",
    "spell",
    "air",
    "away",
    "animal",
    "house",
    "point",
    "page",
    "letter",
    "mother",
    "answer",
    "found",
    "study",
    "still",
    "learn",
    "should",
    "world",
    "high",
    "every",
    "near",
    "add",
    "food",
    "between",
    "own",
    "below",
    "country",
    "plant",
    "last",
    "school",
    "father",
    "keep",
    "tree",
    "never",
    "start",
    "city",
    "earth",
    "eye",
    "light",
    "thought",
    "head",
    "under",
    "story",
    "saw",
    "left",
    "few",
    "while",
    "along",
    "might",
    "close",
    "something",
    "seem",
    "next",
    "hard",
    "open",
    "example",
    "begin",
    "life",
    "always",
    "those",
    "both",
    "paper",
    "together",
    "got",
    "group",
    "often",
    "run",
    "important",
    "until",
    "children",
    "side",
    "feet",
    "car",
    "mile",
    "night",
    "walk",
    "white",
    "sea",
    "began",
    "grow",
    "took",
    "river",
    "four",
    "carry",
    "state",
    "once",
    "book",
    "hear",
    "stop",
    "without",
    "second",
    "later",
    "miss",
    "idea",
    "enough",
    "eat",
    "face",
    "watch",
    "far",
    "really",
    "almost",
    "let",
    "above",
    "girl",
    "sometimes",
    "mountain",
    "cut",
    "young",
    "talk",
    "soon",
    "list",
    "song",
    "being",
    "leave",
    "family",
];

const HARD_WORDS: &[&str] = &[
    "rhythm",
    "quixotic",
    "phenomenon",
    "labyrinth",
    "silhouette",
    "conscientious",
    "onomatopoeia",
    "wristwatch",
    "jazz",
    "squeeze",
    "zigzag",
    "pneumonia",
    "awkward",
    "rhubarb",
    "sphinx",
    "gazebo",
    "banjo",
    "fjord",
    "quinoa",
    "waltz",
    "buzz",
    "vortex",
    "kayak",
    "onyx",
    "papyrus",
];

const PUNCTUATED: &[&str] = &[
    "don't",
    "it's",
    "well-known",
    "self-taught",
    "hello,",
    "world.",
    "wait;",
    "yes!",
    "hmm?",
    "(quick)",
    "\"true\"",
    "e-mail",
    "co-operate",
    "20%",
    "$5",
    "#tag",
    "1/2",
];

const SENTENCES: &[&str] = &[
    "The quick brown fox jumps over the lazy dog.",
    "Pack my box with five dozen liquor jugs.",
    "How vexingly quick daft zebras jump!",
    "Sphinx of black quartz, judge my vow.",
    "Bright vixens jump; dozy fowl quack.",
    "Jackdaws love my big sphinx of quartz.",
    "The five boxing wizards jump quickly.",
    "Waltz, bad nymph, for quick jigs vex.",
    "Quick zephyrs blow, vexing daft Jim.",
    "Two driven jocks help fax my big quiz.",
];

/// A short quote for quote mode. `(text, author)`.
pub struct Quote {
    pub text: &'static str,
    pub author: &'static str,
}

pub const QUOTES: &[Quote] = &[
    Quote {
        text: "The only way to do great work is to love what you do.",
        author: "Steve Jobs",
    },
    Quote {
        text: "Simplicity is the ultimate sophistication.",
        author: "Leonardo da Vinci",
    },
    Quote {
        text: "Talk is cheap. Show me the code.",
        author: "Linus Torvalds",
    },
    Quote {
        text: "Programs must be written for people to read.",
        author: "Harold Abelson",
    },
    Quote {
        text: "Make it work, make it right, make it fast.",
        author: "Kent Beck",
    },
    Quote {
        text: "Perfection is achieved when there is nothing left to take away.",
        author: "Antoine de Saint-Exupery",
    },
    Quote {
        text: "The best way to predict the future is to invent it.",
        author: "Alan Kay",
    },
    Quote {
        text: "Well begun is half done.",
        author: "Aristotle",
    },
    Quote {
        text: "Practice does not make perfect. Only perfect practice makes perfect.",
        author: "Vince Lombardi",
    },
    Quote {
        text: "Speed is useful only if you are running in the right direction.",
        author: "John H. Johnson",
    },
    Quote {
        text: "Little by little, one travels far.",
        author: "J. R. R. Tolkien",
    },
    Quote {
        text: "A ship in harbor is safe, but that is not what ships are built for.",
        author: "John A. Shedd",
    },
];

/// A tiny deterministic xorshift PRNG so prompt generation is reproducible
/// from a seed (tests and retries get the same prompt for the same seed).
#[derive(Debug, Clone, Copy)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() % bound as u64) as usize
        }
    }
}

/// Build the text to type for a spec. Time and Words modes draw words
/// deterministically from the pools (mostly common words, spiced with
/// punctuation and the occasional hard word).
pub fn build_prompt(spec: &PromptSpec, seed: u64) -> String {
    match spec {
        PromptSpec::Quote(index) => quote_prompt(*index),
        PromptSpec::Time(_) | PromptSpec::Words(_) => word_prompt(spec, seed),
    }
}

fn quote_prompt(index: usize) -> String {
    let quote = &QUOTES[index % QUOTES.len()];
    quote.text.to_string()
}

fn word_prompt(spec: &PromptSpec, seed: u64) -> String {
    let target_words = match spec {
        PromptSpec::Time(d) => {
            // ~55 wpm sustained plus headroom; a fast typist should never
            // run out before the timer.
            (d.as_secs() as usize * 2 + 20).max(40)
        }
        PromptSpec::Words(n) => *n,
        PromptSpec::Quote(_) => unreachable!("handled by build_prompt"),
    };
    let mut rng = Rng::new(seed);
    let mut words: Vec<&str> = Vec::with_capacity(target_words);
    while words.len() < target_words {
        let roll = rng.below(100);
        let word = if words.len().is_multiple_of(17) && !PUNCTUATED.is_empty() && roll < 30 {
            PUNCTUATED[rng.below(PUNCTUATED.len())]
        } else if roll < 4 {
            // A sentence replaces a contiguous group of words so the word
            // budget stays intact.
            let sentence = SENTENCES[rng.below(SENTENCES.len())];
            let sentence_words = sentence.split_whitespace().count();
            if words.len() + sentence_words > target_words {
                continue;
            }
            words.extend(std::iter::repeat_n("", sentence_words - 1));
            sentence
        } else if roll < 8 {
            HARD_WORDS[rng.below(HARD_WORDS.len())]
        } else {
            COMMON_WORDS[rng.below(COMMON_WORDS.len())]
        };
        words.push(word);
    }
    // Capitalize the first word of each sentence chunk. Empty placeholder
    // slots (from multi-word sentences) are skipped, keeping the total word
    // count exactly at the target.
    let mut text = String::with_capacity(target_words * 6);
    let mut capitalize = true;
    let mut emitted = 0usize;
    for word in &words {
        if word.is_empty() {
            continue;
        }
        if capitalize {
            if let Some(first) = word.chars().next() {
                text.extend(first.to_uppercase());
                text.push_str(&word[first.len_utf8()..]);
            } else {
                text.push_str(word);
            }
        } else {
            text.push_str(word);
        }
        capitalize = word.ends_with(['.', '!', '?']);
        emitted += 1;
        if emitted < words.iter().filter(|w| !w.is_empty()).count() {
            text.push(' ');
        }
    }
    text
}

pub fn quote_text(index: usize) -> &'static str {
    QUOTES[index % QUOTES.len()].text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_is_deterministic_per_seed() {
        let a = build_prompt(&PromptSpec::Words(25), 42);
        let b = build_prompt(&PromptSpec::Words(25), 42);
        assert_eq!(a, b);
        let c = build_prompt(&PromptSpec::Words(25), 43);
        assert_ne!(a, c);
    }

    #[test]
    fn words_mode_targets_word_count() {
        let text = build_prompt(&PromptSpec::Words(25), 7);
        assert_eq!(text.split_whitespace().count(), 25);
    }

    #[test]
    fn time_mode_generates_headroom_words() {
        let text = build_prompt(&PromptSpec::Time(Duration::from_secs(15)), 7);
        assert!(text.split_whitespace().count() >= 40);
    }

    #[test]
    fn quote_mode_returns_verbatim_text() {
        let text = build_prompt(&PromptSpec::Quote(0), 7);
        assert_eq!(text, QUOTES[0].text);
        assert_eq!(
            build_prompt(&PromptSpec::Quote(999), 7),
            QUOTES[999 % QUOTES.len()].text
        );
    }

    #[test]
    fn mode_tags_are_stable() {
        assert_eq!(
            PromptSpec::Time(Duration::from_secs(30)).mode_tag(),
            "time-30"
        );
        assert_eq!(PromptSpec::Words(50).mode_tag(), "words-50");
        assert_eq!(PromptSpec::Quote(2).mode_tag(), "quote-2");
    }

    #[test]
    fn prompts_are_printable_ascii_or_common_punct() {
        let text = build_prompt(&PromptSpec::Time(Duration::from_secs(60)), 99);
        assert!(text.chars().all(|c| c.is_ascii_graphic() || c == ' '));
    }

    #[test]
    fn rng_respects_bounds() {
        let mut rng = Rng::new(1);
        for _ in 0..1000 {
            assert!(rng.below(5) < 5);
        }
    }
}
