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

    /// Finds the reconnect button in detected text elements matching any configured phrase.
    /// Narrows the returned bounding box and target_point to the exact matched button word_boxes
    /// so merged lines (e.g. "Cancel Retry") or varying modal heights never misplace the click.
    pub fn find_reconnect_button(
        items: &[DetectedText],
        button_phrases: &[String],
        ignore_phrases: &[String],
    ) -> Option<DetectedText> {
        if button_phrases.is_empty() {
            return None;
        }

        // Score tuple: (exact_line_match, exact_token_count, is_short_line, y_tenths_millis, matched_item)
        // Higher tuple wins.
        let mut best: Option<(u8, u8, i32, i32, DetectedText)> = None;

        for item in items {
            if Self::is_ignored(&item.text, ignore_phrases) {
                continue;
            }
            let trimmed = item.text.trim().to_lowercase();
            if trimmed.is_empty() {
                continue;
            }

            for phrase in button_phrases {
                let target = phrase.trim().to_lowercase();
                if target.is_empty() {
                    continue;
                }
                let target_tokens: Vec<&str> = target.split_whitespace().collect();
                if target_tokens.is_empty() {
                    continue;
                }

                let mut matched_candidate: Option<(u8, u8, i32, DetectedText)> = None;

                if !item.word_boxes.is_empty() {
                    let norm_words: Vec<String> = item
                        .word_boxes
                        .iter()
                        .map(|wb| wb.word.trim().to_lowercase())
                        .collect();

                    // Prefer contiguous span of word_boxes matching target_tokens
                    if norm_words.len() >= target_tokens.len() {
                        for start in 0..=(norm_words.len() - target_tokens.len()) {
                            let all_match = target_tokens.iter().enumerate().all(|(k, tok)| {
                                let w = &norm_words[start + k];
                                w == *tok || Self::word_matches(w, tok)
                            });
                            if all_match {
                                let slice = &item.word_boxes[start..start + target_tokens.len()];
                                let min_x = slice.iter().map(|w| w.bounding_box.min_x).fold(1.0f32, f32::min);
                                let min_y = slice.iter().map(|w| w.bounding_box.min_y).fold(1.0f32, f32::min);
                                let max_x = slice.iter().map(|w| w.bounding_box.max_x).fold(0.0f32, f32::max);
                                let max_y = slice.iter().map(|w| w.bounding_box.max_y).fold(0.0f32, f32::max);
                                let matched_text = slice
                                    .iter()
                                    .map(|w| w.word.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" ");

                                let exact_line = u8::from(trimmed == target);
                                let exact_tokens = u8::from(norm_words.len() == target_tokens.len());
                                let extra_words = -((norm_words.len() - target_tokens.len()) as i32);

                                matched_candidate = Some((
                                    exact_line,
                                    exact_tokens,
                                    extra_words,
                                    DetectedText {
                                        text: matched_text,
                                        bounding_box: NormalizedRect::new(min_x, min_y, max_x, max_y),
                                        target_point: Some(NormalizedPoint {
                                            x: (min_x + max_x) * 0.5,
                                            y: (min_y + max_y) * 0.5,
                                        }),
                                        whole_icon_box: None,
                                        word_boxes: slice.to_vec(),
                                    },
                                ));
                                break;
                            }
                        }
                    }

                    // Fallback if OCR merged tokens without spaces or punctuated a single word
                    if matched_candidate.is_none() && (trimmed == target || trimmed.contains(&target)) {
                        let exact_line = u8::from(trimmed == target);
                        let extra_words = -((norm_words.len().saturating_sub(target_tokens.len())) as i32);
                        matched_candidate = Some((
                            exact_line,
                            0,
                            extra_words,
                            DetectedText {
                                text: item.text.clone(),
                                bounding_box: item.bounding_box.clone(),
                                target_point: Some(NormalizedPoint {
                                    x: item.bounding_box.mid_x(),
                                    y: item.bounding_box.mid_y(),
                                }),
                                whole_icon_box: None,
                                word_boxes: item.word_boxes.clone(),
                            },
                        ));
                    }
                } else if trimmed == target || trimmed.contains(&target) {
                    let exact_line = u8::from(trimmed == target);
                    let word_count = item.text.split_whitespace().count();
                    let extra_words = -((word_count.saturating_sub(target_tokens.len())) as i32);
                    matched_candidate = Some((
                        exact_line,
                        exact_line,
                        extra_words,
                        DetectedText {
                            text: item.text.clone(),
                            bounding_box: item.bounding_box.clone(),
                            target_point: Some(NormalizedPoint {
                                x: item.bounding_box.mid_x(),
                                y: item.bounding_box.mid_y(),
                            }),
                            whole_icon_box: None,
                            word_boxes: Vec::new(),
                        },
                    ));
                }

                if let Some((exact_line, exact_tokens, extra_words, candidate)) = matched_candidate {
                    let y_score = (candidate.bounding_box.mid_y() * 10000.0) as i32;
                    let replace = match &best {
                        None => true,
                        Some((b_el, b_et, b_ew, b_y, _)) => {
                            (exact_line, exact_tokens, extra_words, y_score)
                                > (*b_el, *b_et, *b_ew, *b_y)
                        }
                    };
                    if replace {
                        best = Some((exact_line, exact_tokens, extra_words, y_score, candidate));
                    }
                }
            }
        }

        best.map(|(_, _, _, _, item)| item)
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
                let aligned_x = (top.bounding_box.mid_x() - bottom.bounding_box.mid_x()).abs() < 0.10;
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
                        let raw_words: Vec<&str> = if !item.word_boxes.is_empty() {
                            item.word_boxes.iter().map(|w| w.word.as_str()).collect()
                        } else {
                            item.text.split_whitespace().collect()
                        };
                        raw_words
                            .into_iter()
                            .flat_map(|w| w.split(|c: char| !c.is_alphanumeric()))
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .collect();

                let matched_count = tokens
                    .iter()
                    .filter(|tok| cluster_words.iter().any(|w| Self::word_matches(w, tok)))
                    .count();

                // Also match concatenated alphanumeric text (exact or fuzzy sliding window)
                let joined_cluster: String = cluster_words
                    .iter()
                    .flat_map(|w| w.chars())
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(|c| c.to_lowercase())
                    .collect();
                let joined_kw: String = tokens
                    .iter()
                    .flat_map(|t| t.chars())
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(|c| c.to_lowercase())
                    .collect();
                let joined_match = if !joined_kw.is_empty() {
                    if joined_cluster.contains(&joined_kw) {
                        true
                    } else if joined_kw.len() >= 6 && joined_cluster.chars().count() >= joined_kw.len() {
                        let kw_chars: Vec<char> = joined_kw.chars().collect();
                        let cl_chars: Vec<char> = joined_cluster.chars().collect();
                        let win_len = kw_chars.len();
                        cl_chars.windows(win_len).any(|w| {
                            let s: String = w.iter().collect();
                            Self::edit_distance(&s, &joined_kw) <= 2
                        })
                    } else {
                        false
                    }
                } else {
                    false
                };

                if matched_count == tokens.len() || joined_match {
                    // Use bounding boxes of the matched keyword words only (ignoring trailing numbers/adjacent columns)
                    let mut matched_boxes: Vec<&NormalizedRect> = Vec::new();
                    for item in cluster {
                        if !item.word_boxes.is_empty() {
                            for wb in &item.word_boxes {
                                let parts: Vec<&str> = wb
                                    .word
                                    .split(|c: char| !c.is_alphanumeric())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                let word_alnum: String = wb
                                    .word
                                    .chars()
                                    .filter(|c| c.is_alphanumeric())
                                    .flat_map(|c| c.to_lowercase())
                                    .collect();
                                let matches_part = parts
                                    .iter()
                                    .any(|p| tokens.iter().any(|tok| Self::word_matches(p, tok)));
                                let matches_joined = !joined_kw.is_empty()
                                    && word_alnum.len() >= 4
                                    && (word_alnum.contains(&joined_kw)
                                        || joined_kw.contains(&word_alnum)
                                        || (joined_kw.len() >= 6
                                            && word_alnum.chars().count() >= joined_kw.len()
                                            && word_alnum
                                                .chars()
                                                .collect::<Vec<_>>()
                                                .windows(joined_kw.chars().count())
                                                .any(|w| {
                                                    let s: String = w.iter().collect();
                                                    Self::edit_distance(&s, &joined_kw) <= 2
                                                })));
                                if matches_part || matches_joined {
                                    matched_boxes.push(&wb.bounding_box);
                                }
                            }
                        }
                    }

                    let (min_x, max_x, min_y, max_y) = if !matched_boxes.is_empty() {
                        (
                            matched_boxes.iter().map(|b| b.min_x).fold(1.0f32, f32::min),
                            matched_boxes.iter().map(|b| b.max_x).fold(0.0f32, f32::max),
                            matched_boxes.iter().map(|b| b.min_y).fold(1.0f32, f32::min),
                            matched_boxes.iter().map(|b| b.max_y).fold(0.0f32, f32::max),
                        )
                    } else {
                        (
                            cluster.iter().map(|i| i.bounding_box.min_x).fold(1.0f32, f32::min),
                            cluster.iter().map(|i| i.bounding_box.max_x).fold(0.0f32, f32::max),
                            cluster.iter().map(|i| i.bounding_box.min_y).fold(1.0f32, f32::min),
                            cluster.iter().map(|i| i.bounding_box.max_y).fold(0.0f32, f32::max),
                        )
                    };

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
}
