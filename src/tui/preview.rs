use crate::config;
use crate::session::Session;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use unicode_width::UnicodeWidthChar;

use super::theme::Theme;
use super::utils::{extract_highlight_terms, highlight_spans_with_terms};

/// Cached output of line building + physical layout computation.
/// Avoids recomputing `build_preview_lines` and `wrapped_line_count` every frame.
pub struct PreviewCache {
    session_id: String,
    query: String,
    content_len: usize,
    width: u16,
    lines: Vec<Line<'static>>,
    line_starts: Vec<usize>,
    badge_positions: Vec<usize>,
    first_match_physical: Option<usize>,
    total_physical: usize,
}

pub struct Preview<'a> {
    pub session: Option<&'a Session>,
    /// Mutable scroll: may be updated by auto-scroll before rendering.
    pub scroll: &'a mut u16,
    /// If true, auto-scroll to first match before rendering (reset to false).
    pub auto_scroll: &'a mut bool,
    pub query: &'a str,
    /// Output: physical row positions (pre-scroll, accounting for wrap) for icon overlay.
    pub badge_lines: &'a mut Vec<usize>,
    /// Output: total physical rows (for scrollbar).
    pub total_lines: &'a mut usize,
    /// Output: the clamped scroll value actually used for rendering.
    pub rendered_scroll: &'a mut u16,
    /// Logical line index at top of viewport (persisted across frames for resize stability).
    pub top_logical_line: &'a mut usize,
    pub focused: bool,
    pub theme: &'a Theme,
    /// Cached lines + physical layout. Persists across frames in App.
    pub cache: &'a mut Option<PreviewCache>,
}

impl Widget for Preview<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme;
        let border_color = if self.focused {
            theme.primary
        } else {
            theme.on_surface_variant
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" Preview ");

        let Some(session) = self.session else {
            let empty = Paragraph::new("No session selected")
                .block(block)
                .style(Style::default().fg(theme.on_surface_variant));
            empty.render(area, buf);
            return;
        };

        let inner_width = block.inner(area).width;

        // Rebuild cache if session, query, content, or width changed
        let cache_valid = self.cache.as_ref().is_some_and(|c| {
            c.session_id == session.id
                && c.query == self.query
                && c.content_len == session.content.len()
                && c.width == inner_width
        });

        if !cache_valid {
            let agent_color = config::get_agent_config(&session.agent)
                .map(|c| parse_hex_color(c.color))
                .unwrap_or(theme.on_surface);
            let agent_badge = config::get_agent_config(&session.agent)
                .map(|c| c.badge)
                .unwrap_or(&session.agent);

            let (logical_lines, badge_indices, first_match_logical) = build_preview_lines(
                &session.content,
                self.query,
                agent_color,
                agent_badge,
                theme,
            );

            let (lines, line_starts, badge_positions, first_match_physical, total_physical) =
                pre_wrap_lines(
                    logical_lines,
                    &badge_indices,
                    first_match_logical,
                    inner_width as usize,
                );

            *self.cache = Some(PreviewCache {
                session_id: session.id.clone(),
                query: self.query.to_string(),
                content_len: session.content.len(),
                width: inner_width,
                lines,
                line_starts,
                badge_positions,
                first_match_physical,
                total_physical,
            });
        }

        // Borrow cached data
        let c = self.cache.as_ref().unwrap();
        let lines = c.lines.clone();
        *self.badge_lines = c.badge_positions.clone();
        let first_match_physical = c.first_match_physical;
        let total_physical = c.total_physical;

        // On resize (total lines changed), restore scroll to the same logical line
        let old_total = *self.total_lines;
        if !*self.auto_scroll && old_total > 0 && total_physical != old_total {
            let top = (*self.top_logical_line).min(c.line_starts.len().saturating_sub(1));
            *self.scroll = c.line_starts.get(top).copied().unwrap_or(0) as u16;
        }
        *self.total_lines = total_physical;

        // Auto-scroll to first match BEFORE rendering (so first frame is correct)
        if *self.auto_scroll {
            *self.auto_scroll = false;
            if let Some(match_row) = first_match_physical {
                *self.scroll = match_row.saturating_sub(3) as u16;
            }
        }

        // Clamp scroll so it never exceeds content (prevents blank preview after resize)
        let visible_height = block.inner(area).height as usize;
        let max_scroll = total_physical.saturating_sub(visible_height) as u16;
        let scroll = (*self.scroll).min(max_scroll);
        *self.scroll = scroll;
        *self.rendered_scroll = scroll;

        // Update top_logical_line from final scroll position
        *self.top_logical_line = match c.line_starts.binary_search(&(scroll as usize)) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };

        let paragraph = Paragraph::new(lines).block(block).scroll((scroll, 0));

        paragraph.render(area, buf);
    }
}

/// Build styled lines from preview text, matching Python's _render_message logic.
/// Returns (lines, badge_line_indices, first_match_line) where badge_line_indices are the line numbers
/// of assistant first-lines (for icon overlay), and first_match_line is the logical line index
/// of the first highlighted match.
fn build_preview_lines(
    text: &str,
    query: &str,
    agent_color: Color,
    agent_badge: &str,
    theme: &Theme,
) -> (Vec<Line<'static>>, Vec<usize>, Option<usize>) {
    let terms = extract_highlight_terms(query);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut badge_indices: Vec<usize> = Vec::new();
    let mut first_match_line: Option<usize> = None;
    let messages = text.split("\n\n");

    for msg in messages {
        let msg = msg.trim_end();
        if msg.trim().is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        let msg_lines: Vec<&str> = msg.split('\n').collect();
        let is_user = msg.starts_with("» ");
        let mut first_line = true;
        let mut i = 0;

        while i < msg_lines.len() {
            let line = msg_lines[i];

            // Check for code block start: ```language
            if line.starts_with("```") {
                // Collect code block content
                i += 1;
                while i < msg_lines.len() && !msg_lines[i].starts_with("```") {
                    // Render code lines with dim style and indent
                    let code_line = msg_lines[i];
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            code_line.to_string(),
                            Style::default().fg(theme.on_surface_variant),
                        ),
                    ]));
                    i += 1;
                }
                // Skip closing ```
                if i < msg_lines.len() && msg_lines[i].starts_with("```") {
                    i += 1;
                }
                continue;
            }

            if let Some(content) = line.strip_prefix("» ") {
                // User message
                let content = if content.chars().count() > 200 {
                    let truncated: String = content.chars().take(200).collect();
                    format!("{truncated} ...")
                } else {
                    content.to_string()
                };
                let mut spans = vec![Span::styled(
                    "» ".to_string(),
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )];
                let hl = highlight_spans_with_terms(&content, &terms, theme.secondary);
                let has_match = hl.len() > 1;
                spans.extend(hl);
                lines.push(Line::from(spans));
                if has_match && first_match_line.is_none() {
                    first_match_line = Some(lines.len() - 1);
                }
                first_line = false;
            } else if line == "..." {
                lines.push(Line::from(Span::styled(
                    "   ⋯".to_string(),
                    Style::default().fg(theme.on_surface_variant),
                )));
            } else if line.starts_with("...") {
                lines.push(Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(theme.on_surface_variant),
                )));
            } else if line.starts_with("  ") || (!is_user && !line.is_empty()) {
                // Assistant response
                if first_line {
                    let mut content = line.trim_start();
                    // If first line content is empty (e.g. text starts with \n),
                    // pull the next non-empty line into the badge line.
                    if content.is_empty() {
                        while i + 1 < msg_lines.len() {
                            let next = msg_lines[i + 1].trim_start();
                            if next.is_empty() || next == "..." {
                                i += 1;
                                continue;
                            }
                            if next.starts_with("```") {
                                break;
                            }
                            i += 1;
                            content = next;
                            break;
                        }
                    }
                    badge_indices.push(lines.len());
                    // Leave space for icon overlay: "   " (3 chars) + badge + content
                    let mut spans = vec![
                        Span::styled("   ".to_string(), Style::default()), // icon space
                        Span::styled(
                            format!("{agent_badge} "),
                            Style::default()
                                .fg(agent_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ];
                    let hl = highlight_spans_with_terms(content, &terms, theme.on_surface);
                    let has_match = hl.len() > 1;
                    spans.extend(hl);
                    lines.push(Line::from(spans));
                    if has_match && first_match_line.is_none() {
                        first_match_line = Some(lines.len() - 1);
                    }
                    first_line = false;
                } else {
                    let spans = highlight_spans_with_terms(line, &terms, theme.on_surface);
                    if spans.len() > 1 && first_match_line.is_none() {
                        first_match_line = Some(lines.len());
                    }
                    lines.push(Line::from(spans));
                }
            } else if !line.is_empty() {
                let spans = highlight_spans_with_terms(line, &terms, theme.on_surface);
                if spans.len() > 1 && first_match_line.is_none() {
                    first_match_line = Some(lines.len());
                }
                lines.push(Line::from(spans));
            }

            i += 1;
        }

        // Add blank line between messages
        lines.push(Line::from(""));
    }

    (lines, badge_indices, first_match_line)
}

/// Character-level pre-wrapping of preview lines.
/// Replaces ratatui's word wrapper to correctly handle CJK text alongside badges.
/// ratatui's `WordWrapper` treats spaceless CJK text as a single "word", causing
/// badges to appear alone on one line with content pushed to the next.
fn pre_wrap_lines(
    lines: Vec<Line<'static>>,
    badge_indices: &[usize],
    first_match_line: Option<usize>,
    max_width: usize,
) -> (
    Vec<Line<'static>>,
    Vec<usize>,
    Vec<usize>,
    Option<usize>,
    usize,
) {
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut line_starts: Vec<usize> = Vec::with_capacity(lines.len());
    let mut badge_positions: Vec<usize> = Vec::new();
    let mut first_match_physical: Option<usize> = None;

    for (logical_idx, line) in lines.into_iter().enumerate() {
        let physical_start = out.len();
        line_starts.push(physical_start);

        if badge_indices.contains(&logical_idx) {
            badge_positions.push(physical_start);
        }
        if first_match_line == Some(logical_idx) {
            first_match_physical = Some(physical_start);
        }

        // Fast path: line fits in one row
        if max_width == 0 || line.width() <= max_width {
            out.push(line);
            continue;
        }

        // Character-level wrapping
        char_wrap_line(line, max_width, &mut out);
    }

    let total = out.len();
    (
        out,
        line_starts,
        badge_positions,
        first_match_physical,
        total,
    )
}

/// Decompose a Line into (char, Style) pairs and re-partition into rows of max_width cells.
fn char_wrap_line(line: Line<'static>, max_width: usize, out: &mut Vec<Line<'static>>) {
    let mut chars: Vec<(char, Style)> = Vec::new();
    for span in line.spans {
        let style = span.style;
        for c in span.content.chars() {
            chars.push((c, style));
        }
    }

    if chars.is_empty() {
        out.push(Line::from(""));
        return;
    }

    let mut row_start = 0;
    let mut width: usize = 0;

    for i in 0..chars.len() {
        let cw = chars[i].0.width().unwrap_or(0);
        if width > 0 && width + cw > max_width {
            out.push(build_line_from_styled_chars(&chars[row_start..i]));
            row_start = i;
            width = cw;
        } else {
            width += cw;
        }
    }

    if row_start < chars.len() {
        out.push(build_line_from_styled_chars(&chars[row_start..]));
    }
}

/// Reconstruct a Line from a slice of (char, Style) pairs, merging adjacent same-style chars.
fn build_line_from_styled_chars(chars: &[(char, Style)]) -> Line<'static> {
    if chars.is_empty() {
        return Line::from("");
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut text = String::new();
    let mut style = chars[0].1;
    for &(c, s) in chars {
        if s == style {
            text.push(c);
        } else {
            spans.push(Span::styled(std::mem::take(&mut text), style));
            text.push(c);
            style = s;
        }
    }
    if !text.is_empty() {
        spans.push(Span::styled(text, style));
    }
    Line::from(spans)
}

fn parse_hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::White;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_from(text: &str) -> Line<'static> {
        Line::from(text.to_string())
    }

    fn styled_line(spans: Vec<(&str, Color)>) -> Line<'static> {
        Line::from(
            spans
                .into_iter()
                .map(|(t, c)| Span::styled(t.to_string(), Style::default().fg(c)))
                .collect::<Vec<_>>(),
        )
    }

    /// Helper: wrap a single line and return the number of physical rows.
    fn wrap_count(line: Line<'static>, max_width: usize) -> usize {
        let (out, _, _, _, total) = pre_wrap_lines(vec![line], &[], None, max_width);
        assert_eq!(out.len(), total);
        total
    }

    #[test]
    fn wrap_empty_line() {
        assert_eq!(wrap_count(line_from(""), 80), 1);
    }

    #[test]
    fn wrap_short_ascii() {
        assert_eq!(wrap_count(line_from("hello world"), 80), 1);
    }

    #[test]
    fn wrap_exact_fit() {
        assert_eq!(wrap_count(line_from("abcdefghij"), 10), 1);
    }

    #[test]
    fn wrap_one_over() {
        assert_eq!(wrap_count(line_from("abcdefghijk"), 10), 2);
    }

    #[test]
    fn wrap_double_width_exact() {
        // 5 CJK chars = 10 cells, width 10 → 1 row
        assert_eq!(wrap_count(line_from("你好世界啊"), 10), 1);
    }

    #[test]
    fn wrap_double_width_boundary() {
        // 5 CJK chars = 10 cells, width 9 → char wrap:
        // Row 1: "你好世界" (8 cells, next "啊" would be 10 > 9), Row 2: "啊" (2)
        assert_eq!(wrap_count(line_from("你好世界啊"), 9), 2);
    }

    #[test]
    fn wrap_double_width_odd_boundary() {
        // 3 CJK chars = 6 cells, width 5 → Row 1: "你好" (4), Row 2: "世" (2)
        assert_eq!(wrap_count(line_from("你好世"), 5), 2);
    }

    #[test]
    fn wrap_mixed_ascii_cjk() {
        // "a你b" = 4 cells, width 3 → Row 1: "a你" (3), Row 2: "b" (1)
        assert_eq!(wrap_count(line_from("a你b"), 3), 2);
    }

    #[test]
    fn wrap_cjk_pushes_to_next_row() {
        // "ab你" = 4 cells, width 3 → Row 1: "ab" (2), Row 2: "你" (2)
        assert_eq!(wrap_count(line_from("ab你"), 3), 2);
    }

    #[test]
    fn wrap_badge_with_cjk() {
        // Key test: badge + CJK content should NOT push CJK to next line.
        // "» " (2 cells) + "你好世界" (8 cells) = 10 cells, width 6:
        // Row 1: "» 你好" (6 cells), Row 2: "世界" (4 cells) → 2 rows
        let line = styled_line(vec![("» ", Color::Cyan), ("你好世界", Color::White)]);
        assert_eq!(wrap_count(line, 6), 2);
    }

    #[test]
    fn wrap_zero_width() {
        assert_eq!(wrap_count(line_from("hello"), 0), 1);
    }

    #[test]
    fn wrap_long_line() {
        assert_eq!(wrap_count(line_from(&"a".repeat(100)), 10), 10);
    }

    #[test]
    fn wrap_long_cjk_line() {
        // 20 CJK chars = 40 cells, width 11 → each row fits 5 chars (10 cells) → 4 rows
        assert_eq!(wrap_count(line_from(&"你".repeat(20)), 11), 4);
    }

    #[test]
    fn wrap_width_1_ascii() {
        assert_eq!(wrap_count(line_from("abc"), 1), 3);
    }

    #[test]
    fn wrap_width_1_cjk() {
        // CJK chars are 2 cells wide, max_width=1 — each char alone on a row
        assert_eq!(wrap_count(line_from("你好世"), 1), 3);
    }

    #[test]
    fn wrap_preserves_badge_positions() {
        let lines = vec![
            line_from("user message"),
            line_from("   codex 这是一段很长的中文回复内容需要换行处理"),
        ];
        let badge_indices = vec![1];
        let (_, _, badge_positions, _, _) = pre_wrap_lines(lines, &badge_indices, None, 20);
        // Badge should be on the first physical row of logical line 1
        assert_eq!(badge_positions, vec![1]);
    }

    #[test]
    fn wrap_preserves_styles() {
        let line = styled_line(vec![("hello ", Color::Red), ("world", Color::Blue)]);
        let mut out = Vec::new();
        char_wrap_line(line, 4, &mut out);
        // "hello world" (11 cells) → "hell" "o wo" "rld" at width 4
        assert_eq!(out.len(), 3);
        // First row "hell" should be all Red
        assert_eq!(out[0].spans.len(), 1);
        assert_eq!(out[0].spans[0].content.as_ref(), "hell");
        // Second row "o wo" should be Red "o " + Blue "wo"
        assert_eq!(out[1].spans.len(), 2);
        assert_eq!(out[1].spans[0].content.as_ref(), "o ");
        assert_eq!(out[1].spans[1].content.as_ref(), "wo");
    }

    #[test]
    fn parse_hex_color_valid() {
        assert_eq!(parse_hex_color("#E87B35"), Color::Rgb(232, 123, 53));
        assert_eq!(parse_hex_color("4285F4"), Color::Rgb(66, 133, 244));
    }

    #[test]
    fn parse_hex_color_invalid() {
        assert_eq!(parse_hex_color("xyz"), Color::White);
        assert_eq!(parse_hex_color(""), Color::White);
    }
}
