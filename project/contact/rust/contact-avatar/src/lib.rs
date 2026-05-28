use unicode_segmentation::UnicodeSegmentation;

/// Returns the first grapheme of the string in uppercase.
/// Grapheme is a user-perceived character, which can be any Unicode code point.
///
#[must_use]
pub fn first_grapheme_uppercase<S: AsRef<str>>(s: S) -> Option<String> {
    Some(s.as_ref().trim().graphemes(true).next()?.to_uppercase())
}

/// List of Proton colors defined by designers.
static PROTON_COLORS: [&str; 15] = [
    "#2E8378", // Green-1 (Genoa)
    "#34A48A", // Green-2 (Gossamer)
    "#52CD96", // Green-3 (Mountain Meadow)
    "#51BE50", // Green-4 (Apple)
    "#3F8B8E", // Green-5 (Paradiso)
    "#764AC4", // Purple-1 (Royal Purple)
    "#9E66FC", // Purple-2 (Heliotrope)
    "#9C89FF", // Purple-3 (Melrose)
    "#A1439F", // Purple-4 (Medium Red Violet)
    "#7B3185", // Purple-5 (Ripe Plum)
    "#495EA9", // Blue-1 (Bay of Many)
    "#4E7ABB", // Blue-2 (Cobalt)
    "#4989FF", // Blue-3 (Dodger Blue)
    "#3FB0D9", // Blue-4 (Picton Blue)
    "#4F66DF", // Blue-5 (Royal Blue)
];

/// Returns hexadecimal Proton color based on string value.
///
#[must_use]
pub fn proton_color(name: &str) -> &str {
    let mut hash = 0;
    for c in name.chars() {
        hash = (c as u32 + ((hash << 5) - hash)) % (65537);
    }
    let index = hash as usize % PROTON_COLORS.len();
    PROTON_COLORS[index]
}

/// This is the main data structure that is used to represent the avatar information.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AvatarInformation {
    /// The field represent the first grapheme of the name of the contact
    pub text: String,

    /// The field represent the color of the avatar.
    pub color: String,
}

/// Default avatar information if there is no recipient e.g. in draft.
impl Default for AvatarInformation {
    fn default() -> Self {
        AvatarInformation {
            text: "?".to_string(),
            color: "#A7AAB0".to_string(),
        }
    }
}

impl AvatarInformation {
    /// Returns true if the text is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Returns a new `AvatarInformation` with the given value if the text is empty.
    ///
    #[must_use]
    pub fn or_else<I>(self, value: I) -> Self
    where
        I: Into<Self>,
    {
        if self.is_empty() { value.into() } else { self }
    }

    /// Returns a new `AvatarInformation` with the given value if the text is empty.
    /// Provided value is taken as is, not trimmed nor manipulated in any way, use with causation.
    /// Ideal input for this function would be a string that is one grapheme long.
    ///
    #[must_use]
    pub fn or_else_unchecked<S: AsRef<str>>(self, value: S) -> Self {
        if self.is_empty() {
            let name = value.as_ref();
            let text = name.to_string();
            let color = proton_color(name);

            Self {
                text,
                color: color.to_string(),
            }
        } else {
            self
        }
    }
}

impl<S> From<S> for AvatarInformation
where
    S: AsRef<str>,
{
    fn from(value: S) -> Self {
        let name = value.as_ref();

        let text = first_emoji_grapheme(name)
            .or_else(|| name.unicode_words().find_map(first_grapheme_uppercase))
            .unwrap_or_default();

        let color = proton_color(name);

        Self {
            text,
            color: color.to_string(),
        }
    }
}

fn first_emoji_grapheme(s: &str) -> Option<String> {
    s.trim()
        .graphemes(true)
        .find(|g| {
            g.chars().next().is_some_and(|c| {
                ('\u{1F300}'..='\u{1FAFF}').contains(&c)        // Misc emoji
                    || ('\u{1F600}'..='\u{1F64F}').contains(&c) // Emoticons
                    || ('\u{2700}'..='\u{27BF}').contains(&c) // Dingbats & symbols
            })
        })
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    // --- first_grapheme_uppercase ---

    #[test_case("a" => "A")]
    #[test_case("B" => "B")]
    #[test_case("1" => "1")]
    #[test_case("y̆es" => "Y̆")]
    #[test_case("@user" => "@")]
    #[test_case("🗻∈🌏" => "🗻")]
    #[test_case("\"This is a quote\"" => "\"")]
    #[test_case("🧑‍🔬 Doctor Rebecca" => "🧑‍🔬")]
    fn test_first_grapheme_uppercase(s: &str) -> String {
        first_grapheme_uppercase(s).unwrap_or_default()
    }

    // --- proton_color ---

    #[test]
    fn test_proton_color() {
        assert_eq!(proton_color("John Doe"), "#3F8B8E");
        assert_eq!(proton_color("Jane Doe"), "#2E8378");
        assert_eq!(proton_color("Test"), "#A1439F");
        assert_eq!(proton_color(""), "#2E8378");
    }

    // --- AvatarInformation ---

    #[allow(non_snake_case)]
    #[test_case("John Doe" => "J"; "John Doe uppercase")]
    #[test_case("john doe" => "J"; "John Doe lowercase")]
    #[test_case("John" => "J")]
    #[test_case("" => ""; "empty")]
    #[test_case("J" => "J")]
    #[test_case("John 1Doe" => "J")]
    #[test_case("123 John" => "1")]
    #[test_case("🙂" => "🙂"; "emoji")]
    #[test_case("🙂 John" => "🙂"; "John with emoji")]
    #[test_case("🙂 John Doe" => "🙂")]
    #[test_case("brains@tracyisland.com" => "B")]
    #[test_case("    brains@tracyisland.com" => "B"; "leading spaces")]
    #[test_case("A@test.com" => "A")]
    #[test_case("<brains@tracyisland.com>" => "B"; "brackets")]
    #[test_case("@nolocal.com" => "N")]
    #[test_case("Riri Fifi Loulou" => "R")]
    #[test_case("emojiname@test.com`" => "E")]
    #[test_case("OnePart" => "O")]
    #[test_case("onepart@test.com" => "O")]
    #[test_case("🧑‍🔬 Doctor Rebecca" => "🧑‍🔬")]
    #[test_case("Milti-Part Surname" => "M")] // Name with dashes
    #[test_case("日本人の氏名" => "日")] // Japanese
    #[test_case("ім'я прізвище" => "І")] // Ukrainian (Cyrillic)
    #[test_case("שם משפחה" => "ש")] // Hebrew
    fn test_avatar_text(name: &str) -> String {
        AvatarInformation::from(name).text
    }
}
