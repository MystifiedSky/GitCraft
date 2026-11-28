use crate::app::theme::p;
use crate::app::widgets::REMOVED;
use egui::{Align2, FontId, Id, Stroke, TextBuffer, TextEdit, Ui, WidgetAtomResponse, vec2};
use spellbook::Dictionary;
use std::ops::Range;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Language {
    English,
    Spanish,
}

#[derive(Default)]
struct DictionarySlot {
    started: OnceLock<()>,
    dictionary: OnceLock<Option<Dictionary>>,
}

impl Language {
    fn load(self) -> Result<Dictionary, spellbook::ParseDictionaryError> {
        let (aff, dic) = match self {
            Self::English => (
                include_str!("../../../assets/dictionaries/en_US.aff"),
                include_str!("../../../assets/dictionaries/en_US.dic"),
            ),
            Self::Spanish => (
                include_str!("../../../assets/dictionaries/es_ES.aff"),
                include_str!("../../../assets/dictionaries/es_ES.dic"),
            ),
        };
        Dictionary::new(aff, dic)
    }

    fn dictionary(self, ctx: &egui::Context) -> Option<&'static Dictionary> {
        static ENGLISH: DictionarySlot = DictionarySlot {
            started: OnceLock::new(),
            dictionary: OnceLock::new(),
        };
        static SPANISH: DictionarySlot = DictionarySlot {
            started: OnceLock::new(),
            dictionary: OnceLock::new(),
        };
        let slot = match self {
            Self::English => &ENGLISH,
            Self::Spanish => &SPANISH,
        };
        if let Some(dictionary) = slot.dictionary.get() {
            return dictionary.as_ref();
        }
        if slot.started.set(()).is_ok() {
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let _ = slot.dictionary.set(self.load().ok());
                ctx.request_repaint();
            });
        } else {
            // Another window may have requested the dictionary first.
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        None
    }
}

#[derive(Default)]
pub(super) struct State {
    cached: Option<Checked>,
    menu: Option<Correction>,
}

struct Checked {
    language: Language,
    text: String,
    words: Vec<Range<usize>>,
}

struct Correction {
    range: Range<usize>,
    word: String,
    suggestions: Vec<String>,
}

fn misspellings(dictionary: &Dictionary, text: &str) -> Vec<Range<usize>> {
    static WORDS: OnceLock<regex::Regex> = OnceLock::new();
    let words = WORDS.get_or_init(|| {
        regex::Regex::new(r"https?://[^\s]+|[\p{L}\p{M}\p{N}_]+(?:['’][\p{L}\p{M}]+)*").unwrap()
    });
    let text = &text[..text.floor_char_boundary(32 * 1024)];
    words
        .find_iter(text)
        .filter_map(|m| {
            let word = m.as_str();
            let check = word.len() <= spellbook::MAX_WORD_LEN
                && !word.contains("://")
                && !word.chars().any(|c| c == '_' || c.is_numeric())
                && !word.chars().all(char::is_uppercase);
            (check && !dictionary.check(word)).then(|| m.range())
        })
        .collect()
}

pub(super) enum Field {
    Summary(f32),
    Description(usize),
}

pub(super) fn edit(
    ui: &mut Ui,
    text: &mut String,
    id: Id,
    field: Field,
    language: Option<Language>,
    state: &mut State,
) -> WidgetAtomResponse {
    let size = match field {
        Field::Summary(_) => 15.0,
        Field::Description(_) => 13.5,
    };
    let font = FontId::proportional(size);
    let dictionary = language
        .filter(|_| !text.is_empty())
        .and_then(|lang| lang.dictionary(ui.ctx()));
    let mut layout = |ui: &Ui, buf: &dyn TextBuffer, width: f32| {
        let s = buf.as_str();
        let ink = p().white(0.95);
        let format = egui::TextFormat::simple(font.clone(), ink);
        let Some((dictionary, lang)) = dictionary.zip(language) else {
            return ui.fonts_mut(|fonts| fonts.layout(s.to_owned(), font.clone(), ink, width));
        };
        if state
            .cached
            .as_ref()
            .is_none_or(|old| old.text != s || old.language != lang)
        {
            state.cached = Some(Checked {
                language: lang,
                text: s.to_owned(),
                words: misspellings(dictionary, s),
            });
        }
        let mut job = egui::text::LayoutJob::default();
        let marked = egui::TextFormat {
            underline: Stroke::new(1.0, REMOVED),
            ..format.clone()
        };
        let mut from = 0;
        for word in &state
            .cached
            .as_ref()
            .expect("checked the current text")
            .words
        {
            job.append(&s[from..word.start], 0.0, format.clone());
            job.append(&s[word.clone()], 0.0, marked.clone());
            from = word.end;
        }
        job.append(&s[from..], 0.0, format);
        job.wrap.max_width = width;
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    };
    let mut out = match field {
        Field::Summary(width) => TextEdit::singleline(text)
            .frame(egui::Frame::NONE)
            .hint_text(egui::RichText::new("Commit summary").color(p().white(0.7)))
            .align(Align2::LEFT_CENTER)
            .min_size(vec2(width, 30.0))
            .desired_width(width),
        Field::Description(rows) => TextEdit::multiline(text)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 6)))
            .hint_text(egui::RichText::new("Description").color(p().white(0.7)))
            .desired_rows(rows)
            .desired_width(f32::INFINITY),
    }
    .id(id)
    .font(font.clone())
    .layouter(&mut layout)
    .show(ui);

    if language.is_none() {
        state.menu = None;
    }
    if out.response.secondary_clicked() {
        state.menu = None;
        if let (Some(dict), Some(checked), Some(pointer)) = (
            dictionary,
            state.cached.as_ref(),
            ui.ctx().pointer_hover_pos(),
        ) && checked.text == *text
        {
            let cursor = out.galley.cursor_from_pos(pointer - out.galley_pos);
            let at = text.byte_index_from_char_index(cursor.index).0;
            if let Some(range) = checked
                .words
                .iter()
                .find(|r| r.contains(&at) || r.end == at)
            {
                let word = &text[range.clone()];
                let mut suggestions = Vec::new();
                dict.suggest(word, &mut suggestions);
                suggestions.truncate(6);
                state.menu = Some(Correction {
                    range: range.clone(),
                    word: word.to_owned(),
                    suggestions,
                });
            }
        }
    }
    if let Some(correction) = &state.menu {
        let mut replacement = None;
        out.response.context_menu(|ui| {
            if correction.suggestions.is_empty() {
                ui.label("No spelling suggestions");
            }
            for suggestion in &correction.suggestions {
                if ui.button(suggestion).clicked() {
                    replacement = Some(suggestion.clone());
                    ui.close();
                }
            }
        });
        if let Some(replacement) = replacement
            && text.get(correction.range.clone()) == Some(correction.word.as_str())
        {
            let old = out.cursor_range.unwrap_or_default();
            let mut undo = out.state.undoer();
            undo.add_undo(&(old, text.clone()));
            let end = text[..correction.range.start].chars().count() + replacement.chars().count();
            text.replace_range(correction.range.clone(), &replacement);
            let cursor = egui::text::CCursorRange::one(egui::text::CCursor::new(end));
            out.state.cursor.set_char_range(Some(cursor));
            undo.add_undo(&(cursor, text.clone()));
            out.state.set_undoer(undo);
            out.state.store(ui.ctx(), id);
            out.response.request_focus();
            out.response.mark_changed();
            state.menu = None;
        }
    }
    out.response
}

#[cfg(test)]
#[test]
fn headless_commit_spelling_preserves_unicode_and_suggests_corrections() {
    let dictionary = Language::English.load().expect("English dictionary");
    let text = "Fix teh JSON and retry_count in https://example.test/shoud";
    let marked: Vec<&str> = misspellings(&dictionary, text)
        .into_iter()
        .map(|r| &text[r])
        .collect();
    assert_eq!(marked, ["teh"]);
    let mut suggestions = Vec::new();
    dictionary.suggest("teh", &mut suggestions);
    assert!(suggestions.contains(&"the".into()));
    let spanish = Language::Spanish.load().expect("Spanish dictionary");
    assert!(spanish.check("solución"));
    assert!(!spanish.check("soluccción"));
    // Affix flags are single characters: each rule keeps its own, so one rule's suffix never leaks into another.
    assert!(spanish.check("telepáticamente"));
    assert!(!spanish.check("telepáticísimo"));
}
