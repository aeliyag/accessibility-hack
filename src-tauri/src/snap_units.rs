//! Snap-unit segmentation over a cached OCR word map.
//!
//! Modes: word, line, sentence, paragraph. All operate in screen coordinates.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SnapMode {
    #[default]
    Line,
    Word,
    Sentence,
    Paragraph,
}

impl SnapMode {
    pub fn as_str(self) -> &'static str {
        match self {
            SnapMode::Line => "line",
            SnapMode::Word => "word",
            SnapMode::Sentence => "sentence",
            SnapMode::Paragraph => "paragraph",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "word" => SnapMode::Word,
            "sentence" => SnapMode::Sentence,
            "paragraph" => SnapMode::Paragraph,
            _ => SnapMode::Line,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordBox {
    pub text: String,
    pub confidence: f32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct LineCluster {
    pub words: Vec<WordBox>,
    pub cy: f64,
    pub top: f64,
    pub bottom: f64,
}

/// One highlight region (a word, line, sentence span, or paragraph block).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub word_count: usize,
    pub text: String,
    /// Optional multi-band covers for wrapped sentences (top→bottom order).
    pub bands: Vec<(f64, f64, f64, f64)>, // x,y,w,h per line band
}

fn is_edge_punct(c: char) -> bool {
    matches!(
        c,
        ',' | ';'
            | ':'
            | '.'
            | '!'
            | '?'
            | '"'
            | '\''
            | '“'
            | '”'
            | '‘'
            | '’'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '«'
            | '»'
    )
}

/// Shrink a word box by estimating per-glyph width and trimming edge punctuation.
pub fn trim_punct_box(word: &WordBox) -> WordBox {
    let chars: Vec<char> = word.text.chars().collect();
    if chars.is_empty() {
        return word.clone();
    }
    let mut start = 0usize;
    let mut end = chars.len();
    while start < end && is_edge_punct(chars[start]) {
        start += 1;
    }
    while end > start && is_edge_punct(chars[end - 1]) {
        end -= 1;
    }
    if start == 0 && end == chars.len() {
        return word.clone();
    }
    if start >= end {
        return word.clone();
    }
    let unit = word.width / chars.len() as f64;
    WordBox {
        text: chars[start..end].iter().collect(),
        confidence: word.confidence,
        x: word.x + start as f64 * unit,
        y: word.y,
        width: ((end - start) as f64 * unit).max(2.0),
        height: word.height,
    }
}

pub fn cluster_lines(words: &[WordBox]) -> Vec<LineCluster> {
    let mut sorted: Vec<&WordBox> = words.iter().collect();
    sorted.sort_by(|a, b| {
        (a.y + a.height / 2.0)
            .partial_cmp(&(b.y + b.height / 2.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if sorted.is_empty() {
        return Vec::new();
    }

    let median_h = {
        let mut heights: Vec<f64> = sorted.iter().map(|w| w.height).collect();
        heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        heights[heights.len() / 2]
    };
    let line_tol = (median_h * 0.45).max(6.0);

    let mut lines: Vec<LineCluster> = Vec::new();
    for word in sorted {
        if let Some(line) = lines.last_mut() {
            if (word.y + word.height / 2.0 - line.cy).abs() <= line_tol {
                line.words.push(word.clone());
                let n = line.words.len() as f64;
                line.cy = line.words.iter().map(|w| w.y + w.height / 2.0).sum::<f64>() / n;
                line.top = line.top.min(word.y);
                line.bottom = line.bottom.max(word.y + word.height);
                continue;
            }
        }
        lines.push(LineCluster {
            cy: word.y + word.height / 2.0,
            top: word.y,
            bottom: word.y + word.height,
            words: vec![word.clone()],
        });
    }
    // Estimate a space from ordinary inter-word gaps in this text, rather than
    // assuming tab pixels are fixed across fonts/zoom levels. A >4-space gap
    // ends the horizontal run; reading then continues on the next line.
    let mut spaces = Vec::new();
    let mut glyph_widths = Vec::new();
    for row in &mut lines {
        row.words.sort_by(|a, b| a.x.total_cmp(&b.x));
        for pair in row.words.windows(2) {
            let gap = pair[1].x - pair[0].x - pair[0].width;
            if gap > 0.5 && gap <= median_h {
                spaces.push(gap);
            }
        }
        for word in &row.words {
            glyph_widths.push(word.width / word.text.chars().count().max(1) as f64);
        }
    }
    spaces.sort_by(f64::total_cmp);
    glyph_widths.sort_by(f64::total_cmp);
    let fallback = glyph_widths
        .get(glyph_widths.len() / 2)
        .copied()
        .unwrap_or(median_h / 2.0)
        * 0.55;
    let space = spaces
        .get(spaces.len().saturating_sub(1) / 2)
        .copied()
        .unwrap_or(fallback)
        .max(1.0);
    let mut segments = Vec::new();
    for mut row in lines {
        row.words.sort_by(|a, b| a.x.total_cmp(&b.x));
        let gap_limit = 4.0 * space * ((row.bottom - row.top) / median_h).max(0.75);
        let mut group: Vec<WordBox> = Vec::new();
        for word in row.words {
            if group
                .last()
                .map(|prev| word.x - prev.x - prev.width > gap_limit)
                .unwrap_or(false)
            {
                segments.push(make_line(std::mem::take(&mut group)));
            }
            group.push(word);
        }
        if !group.is_empty() {
            segments.push(make_line(group));
        }
    }
    // Western reading order: complete one aligned column top-to-bottom before
    // moving right. This keeps article body text out of sidebar sentences.
    segments.sort_by(|a, b| a.cy.total_cmp(&b.cy));
    let mut columns: Vec<(f64, Vec<LineCluster>)> = Vec::new();
    for line in segments {
        let left = line.words[0].x;
        let tolerance = (median_h * 2.5).max(40.0);
        let nearest = columns
            .iter()
            .enumerate()
            .filter(|(_, (x, _))| (left - x).abs() <= tolerance)
            .min_by(|(_, (a, _)), (_, (b, _))| (left - a).abs().total_cmp(&(left - b).abs()))
            .map(|(i, _)| i);
        if let Some(i) = nearest {
            columns[i].0 = columns[i].0.min(left);
            columns[i].1.push(line);
        } else {
            columns.push((left, vec![line]));
        }
    }
    columns.sort_by(|a, b| a.0.total_cmp(&b.0));
    columns.into_iter().flat_map(|(_, lines)| lines).collect()
}

fn make_line(words: Vec<WordBox>) -> LineCluster {
    let cy = words.iter().map(|w| w.y + w.height / 2.0).sum::<f64>() / words.len() as f64;
    let top = words.iter().map(|w| w.y).fold(f64::INFINITY, f64::min);
    let bottom = words
        .iter()
        .map(|w| w.y + w.height)
        .fold(f64::NEG_INFINITY, f64::max);
    LineCluster {
        words,
        cy,
        top,
        bottom,
    }
}

fn reading_order<'a>(lines: &'a [LineCluster]) -> Vec<(usize, usize, &'a WordBox)> {
    let mut out = Vec::new();
    for (li, line) in lines.iter().enumerate() {
        for (wi, w) in line.words.iter().enumerate() {
            out.push((li, wi, w));
        }
    }
    out
}

/// Distance from a point to an axis-aligned rect (0 if inside).
pub fn dist_to_rect(cx: f64, cy: f64, x: f64, y: f64, w: f64, h: f64) -> f64 {
    let dx = if cx < x {
        x - cx
    } else if cx > x + w {
        cx - (x + w)
    } else {
        0.0
    };
    let dy = if cy < y {
        y - cy
    } else if cy > y + h {
        cy - (y + h)
    } else {
        0.0
    };
    (dx * dx + dy * dy).sqrt()
}

/// Hit-test actual text bands. The outer bounding box includes other sentences
/// before/after a wrapped sentence and must not capture their pointer events.
pub fn dist_to_unit(unit: &UnitRect, cx: f64, cy: f64) -> f64 {
    if unit.bands.is_empty() {
        dist_to_rect(cx, cy, unit.x, unit.y, unit.width, unit.height)
    } else {
        unit.bands
            .iter()
            .map(|&(x, y, w, h)| dist_to_rect(cx, cy, x, y, w, h))
            .fold(f64::INFINITY, f64::min)
    }
}

pub fn cursor_near_unit(unit: &UnitRect, cx: f64, cy: f64, margin: f64) -> bool {
    dist_to_unit(unit, cx, cy) <= margin
}

fn nearest_word_index(lines: &[LineCluster], cx: f64, cy: f64) -> Option<(usize, usize)> {
    let mut best: Option<(f64, usize, usize)> = None;
    for (li, line) in lines.iter().enumerate() {
        for (wi, w) in line.words.iter().enumerate() {
            let dist = dist_to_rect(cx, cy, w.x, w.y, w.width, w.height);
            // Prefer containment / near-edge, then center distance as tie-break.
            let wcx = w.x + w.width / 2.0;
            let wcy = w.y + w.height / 2.0;
            let score = dist * 1000.0 + (wcy - cy).abs() * 3.0 + (wcx - cx).abs();
            if best.map(|(d, _, _)| score < d).unwrap_or(true) {
                best = Some((score, li, wi));
            }
        }
    }
    best.map(|(_, li, wi)| (li, wi))
}

/// How close the cursor must be to a unit before we snap (else release).
/// Generous enough that hovering a glyph clearly counts; blank space still releases.
pub const UNIT_NEAR_MARGIN: f64 = 28.0;

fn union_boxes(words: &[&WordBox]) -> Option<UnitRect> {
    if words.is_empty() {
        return None;
    }
    let left = words.iter().map(|w| w.x).fold(f64::INFINITY, f64::min);
    let right = words
        .iter()
        .map(|w| w.x + w.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let top = words.iter().map(|w| w.y).fold(f64::INFINITY, f64::min);
    let bottom = words
        .iter()
        .map(|w| w.y + w.height)
        .fold(f64::NEG_INFINITY, f64::max);
    let pad_x = 8.0;
    let pad_y = 3.0;
    Some(UnitRect {
        x: left - pad_x,
        y: top - pad_y,
        width: (right - left + pad_x * 2.0).max(8.0),
        height: (bottom - top + pad_y * 2.0).max(8.0),
        word_count: words.len(),
        text: words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        bands: Vec::new(),
    })
}

fn bands_from_words(words: &[&WordBox], lines: &[LineCluster]) -> Vec<(f64, f64, f64, f64)> {
    // Group selected words back into line bands.
    let mut by_line: Vec<(usize, Vec<&WordBox>)> = Vec::new();
    for w in words {
        let li = lines.iter().position(|line| {
            line.words
                .iter()
                .any(|lw| (lw.x - w.x).abs() < 0.5 && (lw.y - w.y).abs() < 0.5 && lw.text == w.text)
        });
        if let Some(li) = li {
            if let Some(entry) = by_line.iter_mut().find(|(i, _)| *i == li) {
                entry.1.push(*w);
            } else {
                by_line.push((li, vec![*w]));
            }
        }
    }
    by_line.sort_by_key(|(i, _)| *i);
    by_line
        .into_iter()
        .filter_map(|(_, ws)| {
            let left = ws.iter().map(|w| w.x).fold(f64::INFINITY, f64::min);
            let right = ws
                .iter()
                .map(|w| w.x + w.width)
                .fold(f64::NEG_INFINITY, f64::max);
            let top = ws.iter().map(|w| w.y).fold(f64::INFINITY, f64::min);
            let bottom = ws
                .iter()
                .map(|w| w.y + w.height)
                .fold(f64::NEG_INFINITY, f64::max);
            Some((
                left - 6.0,
                top - 2.0,
                (right - left + 12.0).max(4.0),
                (bottom - top + 4.0).max(4.0),
            ))
        })
        .collect()
}

fn looks_like_abbreviation(token: &str) -> bool {
    static ABBREVS: &[&str] = &[
        "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "ave", "approx", "dept", "est", "fig",
        "vol", "vs", "etc", "e.g", "i.e", "u.s", "u.k", "a.m", "p.m",
    ];
    let t = token
        .trim_end_matches(|c: char| is_edge_punct(c) && c != '.')
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if ABBREVS.contains(&t.as_str()) {
        return true;
    }
    // Single capital letter initials: "A." / "U.S." handled via dots count
    if t.len() == 1
        && token
            .trim_start_matches(is_edge_punct)
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
    {
        return true;
    }
    false
}

fn is_decimal_number(token: &str) -> bool {
    // e.g. 3.14 or $12.50
    let stripped: String = token
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();
    let parts: Vec<&str> = stripped.split('.').collect();
    parts.len() == 2
        && !parts[0].is_empty()
        && !parts[1].is_empty()
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].chars().all(|c| c.is_ascii_digit())
}

fn token_ends_sentence(token: &str) -> bool {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return false;
    }
    if looks_like_abbreviation(trimmed) {
        return false;
    }
    // Strip trailing closers after terminator: ."  ?)  !]
    let mut chars: Vec<char> = trimmed.chars().collect();
    while let Some(&c) = chars.last() {
        if matches!(c, '"' | '\'' | '”' | '’' | ')' | ']' | '}' | '»') {
            chars.pop();
        } else {
            break;
        }
    }
    // A decimal has internal periods, never a final sentence period.
    let bare: String = chars.iter().collect();
    if !matches!(chars.last(), Some('.' | '!' | '?')) && is_decimal_number(&bare) {
        return false;
    }
    matches!(chars.last(), Some('.' | '!' | '?'))
}

/// Split reading-order words into sentence spans (inclusive index ranges into `order`).
pub fn segment_sentences(lines: &[LineCluster]) -> Vec<std::ops::Range<usize>> {
    let order = reading_order(lines);
    let mut spans = Vec::new();
    let mut offset = 0;
    for paragraph in segment_paragraphs(lines) {
        let count: usize = lines[paragraph].iter().map(|l| l.words.len()).sum();
        let end = offset + count;
        let mut start = offset;
        for i in offset..end {
            if token_ends_sentence(&order[i].2.text) {
                spans.push(start..i + 1);
                start = i + 1;
            }
        }
        // Missing punctuation does not imply one sentence per visual line.
        // Preserve the remaining visible paragraph fragment across line wraps.
        if start < end {
            spans.push(start..end);
        }
        offset = end;
    }
    spans
}

/// Blank vertical space and first-line indentation delimit paragraphs. Columns
/// have already been ordered separately by cluster_lines.
pub fn segment_paragraphs(lines: &[LineCluster]) -> Vec<std::ops::Range<usize>> {
    if lines.is_empty() {
        return Vec::new();
    }
    let left = |line: &LineCluster| line.words.first().map(|w| w.x).unwrap_or(0.0);
    let height = |line: &LineCluster| line.bottom - line.top;
    let mut gaps: Vec<f64> = lines
        .windows(2)
        .filter_map(|pair| {
            let a = &pair[0];
            let b = &pair[1];
            let gap = b.top - a.bottom;
            (gap >= 0.0 && (left(a) - left(b)).abs() <= 40.0).then_some(gap)
        })
        .collect();
    gaps.sort_by(f64::total_cmp);
    // Use the lower median: blank paragraph gaps must not raise the baseline.
    let normal_gap = gaps
        .get(gaps.len().saturating_sub(1) / 2)
        .copied()
        .unwrap_or(0.0);
    let mut spans = Vec::new();
    let mut start = 0;
    for i in 1..lines.len() {
        let a = &lines[i - 1];
        let b = &lines[i];
        let h = height(a).min(height(b)).max(8.0);
        let different_column = b.cy <= a.cy || (left(a) - left(b)).abs() > (h * 2.5).max(40.0);
        let blank = b.top - a.bottom > normal_gap.min(h * 1.2) + (h * 0.65).max(7.0);
        let indented = left(b) - left(&lines[start]) > (h * 1.1).max(16.0);
        let font_break = height(a).max(height(b)) > h * 1.55;
        if different_column || blank || indented || font_break {
            spans.push(start..i);
            start = i;
        }
    }
    spans.push(start..lines.len());
    spans
}

fn finalize_near(unit: UnitRect, cx: f64, cy: f64, margin: f64) -> Option<UnitRect> {
    if cursor_near_unit(&unit, cx, cy, margin) {
        Some(unit)
    } else {
        None
    }
}

/// Pick the unit under/near the cursor. Returns `None` when the cursor is over
/// blank space (farther than [`UNIT_NEAR_MARGIN`] from any candidate).
pub fn select_unit(lines: &[LineCluster], mode: SnapMode, cx: f64, cy: f64) -> Option<UnitRect> {
    select_unit_with_margin(lines, mode, cx, cy, UNIT_NEAR_MARGIN)
}

pub fn select_unit_with_margin(
    lines: &[LineCluster],
    mode: SnapMode,
    cx: f64,
    cy: f64,
    margin: f64,
) -> Option<UnitRect> {
    if lines.is_empty() {
        return None;
    }
    match mode {
        SnapMode::Word => {
            let (li, wi) = nearest_word_index(lines, cx, cy)?;
            let trimmed = trim_punct_box(&lines[li].words[wi]);
            let pad_x = 4.0;
            let pad_y = 2.0;
            finalize_near(
                UnitRect {
                    x: trimmed.x - pad_x,
                    y: trimmed.y - pad_y,
                    width: trimmed.width + pad_x * 2.0,
                    height: trimmed.height + pad_y * 2.0,
                    word_count: 1,
                    text: lines[li].words[wi].text.clone(),
                    bands: vec![(
                        trimmed.x - pad_x,
                        trimmed.y - pad_y,
                        trimmed.width + pad_x * 2.0,
                        trimmed.height + pad_y * 2.0,
                    )],
                },
                cx,
                cy,
                margin,
            )
        }
        SnapMode::Line => {
            let (li, _) = nearest_word_index(lines, cx, cy)?;
            let refs: Vec<&WordBox> = lines[li].words.iter().collect();
            let mut unit = union_boxes(&refs)?;
            unit.bands = bands_from_words(&refs, lines);
            finalize_near(unit, cx, cy, margin)
        }
        SnapMode::Sentence => {
            let order = reading_order(lines);
            let (li, wi) = nearest_word_index(lines, cx, cy)?;
            let idx = order.iter().position(|(l, w, _)| *l == li && *w == wi)?;
            let spans = segment_sentences(lines);
            let span = spans.into_iter().find(|r| r.contains(&idx))?;
            let words: Vec<&WordBox> = order[span].iter().map(|(_, _, w)| *w).collect();
            let mut unit = union_boxes(&words)?;
            unit.bands = bands_from_words(&words, lines);
            finalize_near(unit, cx, cy, margin)
        }
        SnapMode::Paragraph => {
            let (li, _) = nearest_word_index(lines, cx, cy)?;
            let paras = segment_paragraphs(lines);
            let span = paras.into_iter().find(|r| r.contains(&li))?;
            let mut words: Vec<&WordBox> = Vec::new();
            for line in &lines[span] {
                words.extend(line.words.iter());
            }
            let mut unit = union_boxes(&words)?;
            unit.bands = bands_from_words(&words, lines);
            finalize_near(unit, cx, cy, margin)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, x: f64, y: f64, width: f64, height: f64) -> WordBox {
        WordBox {
            text: text.into(),
            confidence: 0.95,
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn trims_edge_punctuation_from_box() {
        let raw = w("Hello,", 100.0, 50.0, 60.0, 16.0);
        let trimmed = trim_punct_box(&raw);
        assert_eq!(trimmed.text, "Hello");
        assert!(trimmed.width < raw.width);
        assert!(trimmed.x >= raw.x);
    }

    #[test]
    fn word_mode_picks_nearest() {
        let lines = cluster_lines(&[
            w("One", 10.0, 10.0, 30.0, 14.0),
            w("Two", 50.0, 10.0, 30.0, 14.0),
            w("Three", 10.0, 40.0, 40.0, 14.0),
        ]);
        let unit = select_unit(&lines, SnapMode::Word, 55.0, 12.0).unwrap();
        assert_eq!(unit.word_count, 1);
        assert!(unit.x < 55.0 && unit.x + unit.width > 55.0);
    }

    #[test]
    fn sentence_splits_on_terminators_not_abbreviations_or_decimals() {
        let words = vec![
            w("Dr.", 10.0, 10.0, 20.0, 14.0),
            w("Smith", 35.0, 10.0, 40.0, 14.0),
            w("saw", 80.0, 10.0, 25.0, 14.0),
            w("3.14", 110.0, 10.0, 30.0, 14.0),
            w("cats.", 145.0, 10.0, 35.0, 14.0),
            w("Then", 10.0, 40.0, 35.0, 14.0),
            w("he", 50.0, 40.0, 20.0, 14.0),
            w("left!", 75.0, 40.0, 30.0, 14.0),
        ];
        let lines = cluster_lines(&words);
        let spans = segment_sentences(&lines);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].len(), 5); // through cats.
        assert_eq!(spans[1].len(), 3); // Then he left!
    }

    #[test]
    fn sentence_can_span_lines_and_produce_bands() {
        let words = vec![
            w("Hello", 10.0, 10.0, 40.0, 14.0),
            w("there,", 55.0, 10.0, 40.0, 14.0),
            w("friend.", 10.0, 40.0, 45.0, 14.0),
        ];
        let lines = cluster_lines(&words);
        assert_eq!(lines.len(), 2);
        let unit = select_unit(&lines, SnapMode::Sentence, 20.0, 12.0).unwrap();
        assert_eq!(unit.word_count, 3);
        assert!(unit.height > 20.0);
        assert!(unit.bands.len() >= 2);
    }

    #[test]
    fn paragraph_splits_on_blank_gap_and_indent() {
        // Two tight lines, blank gap, then indented line.
        let words = vec![
            w("First", 10.0, 10.0, 40.0, 14.0),
            w("line", 55.0, 10.0, 30.0, 14.0),
            w("Second", 10.0, 28.0, 45.0, 14.0),
            w("line.", 60.0, 28.0, 30.0, 14.0),
            // gap ~40 (blank)
            w("Indented", 40.0, 70.0, 55.0, 14.0),
            w("para.", 100.0, 70.0, 35.0, 14.0),
        ];
        let lines = cluster_lines(&words);
        assert!(lines.len() >= 3);
        let paras = segment_paragraphs(&lines);
        assert!(paras.len() >= 2);
        let unit = select_unit(&lines, SnapMode::Paragraph, 50.0, 15.0).unwrap();
        assert!(unit.word_count >= 2);
        // Cursor on first para should not include indented block if split worked.
        let unit2 = select_unit(&lines, SnapMode::Paragraph, 50.0, 72.0).unwrap();
        assert!(unit2.word_count <= 3);
    }

    #[test]
    fn sentence_without_terminators_preserves_wrapped_fragment() {
        let words = vec![
            w("Hello", 10.0, 10.0, 40.0, 14.0),
            w("there", 55.0, 10.0, 40.0, 14.0),
            w("Friend", 10.0, 40.0, 45.0, 14.0),
            w("again", 60.0, 40.0, 40.0, 14.0),
        ];
        let lines = cluster_lines(&words);
        assert_eq!(lines.len(), 2);
        let spans = segment_sentences(&lines);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].len(), 4);
        let unit = select_unit(&lines, SnapMode::Sentence, 20.0, 12.0).unwrap();
        assert_eq!(unit.word_count, 4);
    }

    #[test]
    fn blank_space_releases_all_modes() {
        let words = vec![
            w("Hello", 10.0, 10.0, 40.0, 14.0),
            w("world.", 55.0, 10.0, 45.0, 14.0),
            w("Next", 10.0, 40.0, 35.0, 14.0),
        ];
        let lines = cluster_lines(&words);
        // Far to the right of the line — blank.
        assert!(select_unit(&lines, SnapMode::Word, 400.0, 12.0).is_none());
        assert!(select_unit(&lines, SnapMode::Line, 400.0, 12.0).is_none());
        assert!(select_unit(&lines, SnapMode::Sentence, 400.0, 12.0).is_none());
        // Far below all lines — blank.
        assert!(select_unit(&lines, SnapMode::Paragraph, 30.0, 300.0).is_none());
        // Between words but still near "Hello" edge should snap if within margin.
        let near = select_unit(&lines, SnapMode::Word, 52.0, 17.0);
        assert!(near.is_some());
    }
    #[test]
    fn wrapped_sentence_selects_same_words_from_every_line() {
        let lines = cluster_lines(&[
            w("Earlier.", 10.0, 10.0, 55.0, 14.0),
            w("This", 70.0, 10.0, 30.0, 14.0),
            w("sentence", 10.0, 34.0, 60.0, 14.0),
            w("continues", 75.0, 34.0, 65.0, 14.0),
            w("here.”", 10.0, 58.0, 42.0, 14.0),
            w("Next.", 57.0, 58.0, 40.0, 14.0),
        ]);
        for (x, y) in [(80.0, 17.0), (20.0, 41.0), (20.0, 65.0)] {
            let unit = select_unit(&lines, SnapMode::Sentence, x, y).unwrap();
            assert_eq!(unit.word_count, 4);
            assert_eq!(unit.bands.len(), 3);
            assert_eq!(unit.text, "This sentence continues here.”");
            assert!(dist_to_unit(&unit, 20.0, 17.0) > 0.0);
        }
        assert_eq!(
            select_unit(&lines, SnapMode::Sentence, 75.0, 65.0)
                .unwrap()
                .word_count,
            1
        );
    }

    #[test]
    fn article_and_sidebar_sentences_do_not_merge() {
        let lines = cluster_lines(&[
            w("Body", 100.0, 10.0, 40.0, 14.0),
            w("wraps", 145.0, 10.0, 40.0, 14.0),
            w("here.", 100.0, 34.0, 40.0, 14.0),
            w("Sidebar", 450.0, 10.0, 50.0, 14.0),
            w("story.", 450.0, 34.0, 40.0, 14.0),
        ]);
        assert_eq!(
            select_unit(&lines, SnapMode::Sentence, 110.0, 40.0)
                .unwrap()
                .word_count,
            3
        );
        assert_eq!(
            select_unit(&lines, SnapMode::Sentence, 465.0, 40.0)
                .unwrap()
                .word_count,
            2
        );
    }

    #[test]
    fn sentence_never_crosses_blank_paragraph_boundary() {
        let lines = cluster_lines(&[
            w("Unfinished", 10.0, 10.0, 75.0, 14.0),
            w("thought", 10.0, 30.0, 50.0, 14.0),
            w("New", 10.0, 78.0, 25.0, 14.0),
            w("paragraph.", 40.0, 78.0, 70.0, 14.0),
        ]);
        let units = segment_sentences(&lines);
        assert_eq!(units, vec![0..2, 2..4]);
    }

    #[test]
    fn decimal_at_sentence_end_and_closing_quotes() {
        assert!(!token_ends_sentence("3.14"));
        assert!(token_ends_sentence("3.14."));
        assert!(token_ends_sentence("done!\""));
        assert!(!token_ends_sentence("Dr."));
    }

    #[test]
    fn gap_over_four_spaces_moves_to_next_line_not_sidebar() {
        let lines = cluster_lines(&[
            w("Read", 10.0, 10.0, 32.0, 14.0),
            w("this", 47.0, 10.0, 32.0, 14.0),
            w("Aside.", 105.0, 10.0, 40.0, 14.0), // 26px > four normal 5px spaces
            w("next", 10.0, 34.0, 32.0, 14.0),
            w("line", 47.0, 34.0, 32.0, 14.0),
            w("end.", 84.0, 34.0, 32.0, 14.0),
        ]);
        let unit = select_unit(&lines, SnapMode::Sentence, 20.0, 17.0).unwrap();
        assert_eq!(unit.word_count, 5);
        assert_eq!(unit.bands.len(), 2);
        assert_eq!(
            select_unit(&lines, SnapMode::Sentence, 120.0, 17.0)
                .unwrap()
                .word_count,
            1
        );
    }
    #[test]
    fn vision_fixture_selects_wrapped_sentences_at_every_screen_height() {
        // Actual Vision output from an original synthetic 3024x1964 test page.
        // Three copies of a 23-word paragraph at logical y=60,420,860.
        let mut words: Vec<WordBox> =
            serde_json::from_str(include_str!("../test-data/retina-ocr.json")).unwrap();
        for word in &mut words {
            word.x /= 2.0;
            word.y /= 2.0;
            word.width /= 2.0;
            word.height /= 2.0;
        }
        let lines = cluster_lines(&words);
        for top in [60.0, 420.0, 860.0] {
            for line in [0.0, 26.0] {
                let unit =
                    select_unit(&lines, SnapMode::Sentence, 220.0, top + line + 8.0).unwrap();
                assert_eq!(unit.word_count, 14, "first sentence y={top}, line={line}");
                assert_eq!(unit.bands.len(), 2);
            }
            let paragraph = select_unit(&lines, SnapMode::Paragraph, 220.0, top + 60.0).unwrap();
            assert_eq!(paragraph.word_count, 23, "paragraph y={top}");
            assert_eq!(paragraph.bands.len(), 3);
            assert_eq!(
                select_unit(&lines, SnapMode::Sentence, 220.0, top + 60.0)
                    .unwrap()
                    .word_count,
                9
            );
        }
    }
}
