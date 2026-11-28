use super::*;
use crate::models::{DetectedWord, NormalizedPoint, NormalizedRect};

fn make_item(text: &str) -> DetectedText {
    let words = text
        .split_whitespace()
        .map(|w| DetectedWord {
            word: w.to_string(),
            bounding_box: NormalizedRect::new(0.0, 0.0, 0.1, 0.1),
        })
        .collect();

    DetectedText {
        text: text.to_string(),
        bounding_box: NormalizedRect::new(0.0, 0.0, 0.2, 0.1),
        target_point: Some(NormalizedPoint { x: 0.1, y: 0.05 }),
        whole_icon_box: None,
        word_boxes: words,
    }
}

#[test]
fn test_find_reconnect_button_english() {
    let items = vec![make_item("Error Code: 277"), make_item("Reconnect")];
    let phrases = vec!["Reconnect".to_string(), "Połącz ponownie".to_string()];
    let ignores = vec![];

    let found = Classifier::find_reconnect_button(&items, &phrases, &ignores);
    assert!(found.is_some());
    assert_eq!(found.unwrap().text, "Reconnect");
}

#[test]
fn test_find_reconnect_button_polish() {
    let items = vec![
        make_item("Utracono połączenie"),
        make_item("Połącz ponownie"),
    ];
    let phrases = vec!["Reconnect".to_string(), "Połącz ponownie".to_string()];
    let ignores = vec![];

    let found = Classifier::find_reconnect_button(&items, &phrases, &ignores);
    assert!(found.is_some());
    assert_eq!(found.unwrap().text, "Połącz ponownie");
}

#[test]
fn test_find_reconnect_button_spanish() {
    let items = vec![make_item("Desconectado"), make_item("Reconectar")];
    let phrases = vec!["Reconnect".to_string(), "Reconectar".to_string()];
    let ignores = vec![];

    let found = Classifier::find_reconnect_button(&items, &phrases, &ignores);
    assert!(found.is_some());
    assert_eq!(found.unwrap().text, "Reconectar");
}

#[test]
fn test_find_reconnect_button_german() {
    let items = vec![make_item("Verbindung getrennt"), make_item("Erneut verbinden")];
    let phrases = vec!["Reconnect".to_string(), "Erneut verbinden".to_string()];
    let ignores = vec![];

    let found = Classifier::find_reconnect_button(&items, &phrases, &ignores);
    assert!(found.is_some());
    assert_eq!(found.unwrap().text, "Erneut verbinden");
}

#[test]
fn test_find_reconnect_button_ignores_ignored() {
    let items = vec![make_item("Voice Chat Reconnect")];
    let phrases = vec!["Reconnect".to_string()];
    let ignores = vec!["voice chat".to_string()];

    let found = Classifier::find_reconnect_button(&items, &phrases, &ignores);
    assert!(found.is_none());
}

#[test]
fn test_find_event_target_handles_merged_and_fuzzy_words() {
    let items = vec![make_item("TurkeyHunt")];
    let keywords = vec!["turkey hunt".to_string()];
    let found = Classifier::find_event_target(&items, &keywords, 0.40);
    assert!(found.is_some());
    assert_eq!(found.unwrap().text, "TurkeyHunt");

    let apostrophe = vec![make_item("Turkey'Hunt 22,519")];
    assert!(Classifier::find_event_target(&apostrophe, &keywords, 0.40).is_some());

    let typo_merged = vec![make_item("TurkoyHunt 22,519")];
    assert!(Classifier::find_event_target(&typo_merged, &keywords, 0.40).is_some());
}

#[test]
fn test_find_reconnect_button_narrows_merged_line_and_ignores_body_text() {
    let body = DetectedText {
        text: "You were disconnected. Please reconnect to continue.".to_string(),
        bounding_box: NormalizedRect::new(0.25, 0.40, 0.75, 0.46),
        target_point: None,
        whole_icon_box: None,
        word_boxes: vec![
            DetectedWord { word: "Please".into(), bounding_box: NormalizedRect::new(0.42, 0.40, 0.48, 0.46) },
            DetectedWord { word: "reconnect".into(), bounding_box: NormalizedRect::new(0.49, 0.40, 0.58, 0.46) },
        ],
    };

    // Merged bottom button row: "Opuść" on the left, "Połącz ponownie" on the right
    let merged_buttons = DetectedText {
        text: "Opuść Połącz ponownie".to_string(),
        bounding_box: NormalizedRect::new(0.28, 0.72, 0.72, 0.78),
        target_point: None,
        whole_icon_box: None,
        word_boxes: vec![
            DetectedWord { word: "Opuść".into(), bounding_box: NormalizedRect::new(0.28, 0.72, 0.40, 0.78) },
            DetectedWord { word: "Połącz".into(), bounding_box: NormalizedRect::new(0.52, 0.72, 0.61, 0.78) },
            DetectedWord { word: "ponownie".into(), bounding_box: NormalizedRect::new(0.62, 0.72, 0.72, 0.78) },
        ],
    };

    let items = vec![body, merged_buttons];
    let phrases = vec!["Reconnect".to_string(), "Połącz ponownie".to_string()];
    let found = Classifier::find_reconnect_button(&items, &phrases, &[]).expect("button found");

    assert_eq!(found.text, "Połącz ponownie");
    let pt = found.click_point();
    // Exact center of [0.52..0.72] x [0.72..0.78] is (0.62, 0.75), not whole-line midpoint (0.50, 0.75) or body text (y=0.43)
    assert!((pt.x - 0.62).abs() < 0.001, "expected x ~ 0.62, got {}", pt.x);
    assert!((pt.y - 0.75).abs() < 0.001, "expected y ~ 0.75, got {}", pt.y);
}

#[test]
fn test_find_reconnect_button_no_false_positive_on_short_fragments_with_cjk_phrases() {
    let phrases = vec![
        "Reconnect".to_string(),
        "Retry".to_string(),
        "Połącz ponownie".to_string(),
        "다시 연결".to_string(),
        "再接続".to_string(),
        "重新连接".to_string(),
        "Kết nối lại".to_string(),
    ];
    let false_fragments = [
        "A u", "0.9 Q", "P p", "C ar", "N. up", "P 99", "09 37", "Lö I", "36 8m", "0 n",
        "M e", "Q p", "A to", "u 25", "is in", "0 0",
    ];
    let items: Vec<DetectedText> = false_fragments.iter().map(|t| make_item(t)).collect();
    let found = Classifier::find_reconnect_button(&items, &phrases, &[]);
    assert!(found.is_none(), "unexpected false positive: {:?}", found.map(|f| f.text));

    // Real Korean button must still match
    let real_korean = vec![make_item("다시 연결")];
    let found_ko = Classifier::find_reconnect_button(&real_korean, &phrases, &[]);
    assert_eq!(found_ko.map(|f| f.text).as_deref(), Some("다시 연결"));
}
