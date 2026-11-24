use crate::models::{DetectedText, NormalizedPoint, NormalizedRect};

pub struct Classifier;

impl Classifier {
    /// Returns true if the text matches any non-fatal phrase that must be ignored
    pub fn is_ignored(text: &str, ignore_phrases: &[String]) -> bool {
        let lower = text.trim().to_lowercase();
        if lower.is_empty() {
            return false;
        }
        ignore_phrases.iter().any(|p| {
            let cl = p.trim().to_lowercase();
            !cl.is_empty() && lower.contains(&cl)
        })
    }

    /// Returns true if the text indicates a fatal disconnect requiring action
    pub fn is_fatal_disconnect(text: &str, disconnect_keywords: &[String], ignore_phrases: &[String]) -> bool {
        if Self::is_ignored(text, ignore_phrases) {
            return false;
        }
        let lower = text.trim().to_lowercase();
        if lower.is_empty() {
            return false;
        }
        disconnect_keywords.iter().any(|kw| {
            let k = kw.trim().to_lowercase();
            !k.is_empty() && lower.contains(&k)
        })
    }

    /// Finds the reconnect button in detected text elements matching any configured phrase
    pub fn find_reconnect_button(
        items: &[DetectedText],
        button_phrases: &[String],
        ignore_phrases: &[String],
    ) -> Option<DetectedText> {
        if button_phrases.is_empty() {
            return None;
        }

        for phrase in button_phrases {
            let target = phrase.trim().to_lowercase();
            if target.is_empty() {
                continue;
            }

            for item in items {
                if Self::is_ignored(&item.text, ignore_phrases) {
                    continue;
                }
                let trimmed = item.text.trim().to_lowercase();
                if trimmed == target || trimmed.contains(&target) {
                    return Some(item.clone());
                }

                // Check individual words or tokens if target matches with fuzzy tolerance
                let target_tokens: Vec<&str> = target.split_whitespace().collect();
                if target_tokens.len() == 1 {
                    let tok = target_tokens[0];
                    if item.word_boxes.iter().any(|wb| {
                        let w = wb.word.trim().to_lowercase();
                        w == tok || Self::word_matches(&w, tok)
                    }) {
                        return Some(item.clone());
                    }
                } else if !target_tokens.is_empty() {
                    let item_words: Vec<String> = if !item.word_boxes.is_empty() {
                        item.word_boxes.iter().map(|w| w.word.to_lowercase()).collect()
                    } else {
                        item.text.split_whitespace().map(|w| w.to_lowercase()).collect()
                    };
                    let matched_count = target_tokens
                        .iter()
                        .filter(|tok| item_words.iter().any(|w| w == *tok || Self::word_matches(w, tok)))
                        .count();
                    if matched_count == target_tokens.len() {
                        return Some(item.clone());
                    }
                }
            }
        }
        None
    }

    /// Levenshtein edit distance between two strings
    pub fn edit_distance(s1: &str, s2: &str) -> usize {
        let a: Vec<char> = s1.chars().collect();
        let b: Vec<char> = s2.chars().collect();
        if a.is_empty() || b.is_empty() {
            return a.len().max(b.len());
        }

        let mut dist = vec![vec![0usize; b.len() + 1]; a.len() + 1];
        for i in 0..=a.len() {
            dist[i][0] = i;
        }
        for j in 0..=b.len() {
            dist[0][j] = j;
        }

        for i in 1..=a.len() {
            for j in 1..=b.len() {
                if a[i - 1] == b[j - 1] {
                    dist[i][j] = dist[i - 1][j - 1];
                } else {
                    dist[i][j] = 1 + dist[i - 1][j].min(dist[i][j - 1]).min(dist[i - 1][j - 1]);
                }
            }
        }
        dist[a.len()][b.len()]
    }

    /// Fuzzy word matcher accounting for stylized font OCR misreadings (e.g. Turkcy, Hung)
    pub fn word_matches(ocr_word: &str, target_token: &str) -> bool {
        let ocr: String = ocr_word.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
        let target: String = target_token.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
        if ocr.is_empty() || target.is_empty() {
            return false;
        }
        if ocr == target {
            return true;
        }

        let norm_ocr = ocr.replace('c', "e").replace('g', "t");
        let norm_target = target.replace('c', "e").replace('g', "t");
        if norm_ocr == norm_target {
            return true;
        }

        let dist = Self::edit_distance(&norm_ocr, &norm_target);
        if target.len() <= 3 {
            dist == 0
        } else if target.len() <= 5 {
            dist <= 1
        } else {
            dist <= 2
        }
    }

    /// Identifies event icons on the left UI panel (min_x < max_x)
    pub fn find_event_target(
        items: &[DetectedText],
        keywords: &[String],
        max_normalized_x: f32,
    ) -> Option<DetectedText> {
        let candidate_items: Vec<&DetectedText> =
            items.iter().filter(|i| i.bounding_box.min_x < max_normalized_x).collect();

        if candidate_items.is_empty() {
            return None;
        }

        // Group candidate lines into vertically stacked button clusters
        let mut clusters: Vec<Vec<&DetectedText>> = Vec::new();
        let mut used = std::collections::HashSet::new();
        let mut sorted = candidate_items.clone();
        sorted.sort_by(|a, b| a.bounding_box.min_y.partial_cmp(&b.bounding_box.min_y).unwrap());

        for i in 0..sorted.len() {
            if used.contains(&i) {
                continue;
            }
            let mut cluster = vec![sorted[i]];
            used.insert(i);

            for j in (i + 1)..sorted.len() {
                if used.contains(&j) {
                    continue;
                }
                let top = cluster.last().unwrap();
                let bottom = sorted[j];
                let aligned_x = (top.bounding_box.mid_x() - bottom.bounding_box.mid_x()).abs() < 0.06;
                let vertical_gap = bottom.bounding_box.min_y - top.bounding_box.max_y;

                if aligned_x && vertical_gap >= -0.02 && vertical_gap < 0.06 {
                    cluster.push(bottom);
                    used.insert(j);
                    break;
                }
            }
            clusters.push(cluster);
        }

        for kw in keywords {
            let tokens: Vec<String> = kw
                .split_whitespace()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            if tokens.is_empty() {
                continue;
            }

            for cluster in &clusters {
                let cluster_words: Vec<String> = cluster
                    .iter()
                    .flat_map(|item| -> Vec<String> {
                        if !item.word_boxes.is_empty() {
                            item.word_boxes.iter().map(|w| w.word.clone()).collect()
                        } else {
                            item.text.split_whitespace().map(|s| s.to_string()).collect()
                        }
                    })
                    .collect();

                let matched_count = tokens
                    .iter()
                    .filter(|tok| cluster_words.iter().any(|w| Self::word_matches(w, tok)))
                    .count();

                if matched_count == tokens.len() {
                    // Match found! Calculate icon centroid directly above the text label
                    let min_x = cluster.iter().map(|i| i.bounding_box.min_x).fold(1.0f32, f32::min);
                    let max_x = cluster.iter().map(|i| i.bounding_box.max_x).fold(0.0f32, f32::max);
                    let min_y = cluster.iter().map(|i| i.bounding_box.min_y).fold(1.0f32, f32::min);
                    let max_y = cluster.iter().map(|i| i.bounding_box.max_y).fold(0.0f32, f32::max);

                    let label_height = (max_y - min_y).max(0.01);
                    let is_multi_line = cluster.len() > 1;

                    // In top-down coordinates, the icon is directly ABOVE the text (lower Y)
                    let icon_center_y = if is_multi_line {
                        (min_y - label_height * 1.0).max(0.01)
                    } else {
                        (min_y - label_height * 1.25).max(0.01)
                    };
                    let icon_center_x = (min_x + max_x) * 0.5;

                    let whole_icon_box = NormalizedRect::new(
                        min_x.min(icon_center_x - label_height * 1.2),
                        (min_y - label_height * 2.5).max(0.0),
                        max_x.max(icon_center_x + label_height * 1.2),
                        max_y,
                    );

                    let combined_text = cluster.iter().map(|i| i.text.as_str()).collect::<Vec<_>>().join(" ");

                    return Some(DetectedText {
                        text: combined_text,
                        bounding_box: NormalizedRect::new(min_x, min_y, max_x, max_y),
                        target_point: Some(NormalizedPoint {
                            x: icon_center_x,
                            y: icon_center_y,
                        }),
                        whole_icon_box: Some(whole_icon_box),
                        word_boxes: cluster.iter().flat_map(|i| i.word_boxes.clone()).collect(),
                    });
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
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
}
