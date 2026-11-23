//! The search terms suggested under the search box while typing. ↑ and ↓ go through them
//! (the box shows the one picked, and Enter searches for it), and past either end back to
//! what was typed, like YouTube Music's search box.

/// The suggestions for what is typed, and the one picked with the arrows.
#[derive(Debug, Default)]
pub struct Suggestions {
    terms: Vec<String>,
    /// Bumped on every keystroke, so late suggestions for old text are dropped.
    request: u64,
    /// The suggestion picked with the arrows; `None` is the text typed.
    highlighted: Option<usize>,
    /// What the user typed, to go back to past the ends.
    typed: String,
}

impl Suggestions {
    /// The user typed `query`: suggestions for it are asked for with the request number
    /// returned, and the ones shown stay until they come.
    pub fn typed(&mut self, query: &str) -> u64 {
        self.request += 1;
        self.typed = query.to_string();
        self.highlighted = None;
        self.request
    }

    /// No suggestions: nothing typed, or the search ran.
    pub fn clear(&mut self) {
        self.request += 1;
        self.terms.clear();
        self.highlighted = None;
    }

    /// Shows `terms`, unless the text changed since request `request`.
    pub fn loaded(&mut self, request: u64, terms: Vec<String>) {
        if request == self.request {
            self.terms = terms;
            self.highlighted = None;
        }
    }

    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    /// Moves the pick `step` places down (up if negative), through the typed text past the
    /// ends. Returns what the search box shows now, or `None` with no suggestions (the arrows
    /// then do what they do elsewhere).
    pub fn step(&mut self, step: i32) -> Option<String> {
        let count = self.terms.len();
        if count == 0 {
            return None;
        }
        // The typed text is one more place, after the last suggestion.
        let places = count as i64 + 1;
        let at = self.highlighted.unwrap_or(count) as i64;
        let next = (at + i64::from(step)).rem_euclid(places) as usize;
        self.highlighted = (next < count).then_some(next);
        Some(self.terms.get(next).unwrap_or(&self.typed).clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(typed: &str, terms: &[&str]) -> Suggestions {
        let mut suggestions = Suggestions::default();
        let request = suggestions.typed(typed);
        suggestions.loaded(request, terms.iter().map(|term| term.to_string()).collect());
        suggestions
    }

    #[test]
    fn the_arrows_go_through_the_suggestions_and_back_to_what_was_typed() {
        let mut suggestions = shown("ad", &["ado", "adele"]);
        assert_eq!(suggestions.step(1).as_deref(), Some("ado"));
        assert_eq!(suggestions.step(1).as_deref(), Some("adele"));
        assert_eq!(suggestions.highlighted(), Some(1));
        assert_eq!(suggestions.step(1).as_deref(), Some("ad"), "past the end");
        assert_eq!(suggestions.highlighted(), None);
        assert_eq!(
            suggestions.step(-1).as_deref(),
            Some("adele"),
            "up from the text"
        );
    }

    #[test]
    fn with_no_suggestions_the_arrows_are_left_alone() {
        let mut suggestions = shown("ad", &[]);
        assert_eq!(suggestions.step(1), None);
    }

    #[test]
    fn typing_again_drops_the_pick_and_late_answers() {
        let mut suggestions = shown("ad", &["ado"]);
        suggestions.step(1);
        let old = suggestions.typed("ado");
        assert_eq!(suggestions.highlighted(), None);
        let _ = suggestions.typed("ado ");
        suggestions.loaded(old, vec!["late".to_string()]);
        assert_eq!(suggestions.terms(), ["ado"]);
        suggestions.clear();
        assert!(suggestions.terms().is_empty());
    }
}
