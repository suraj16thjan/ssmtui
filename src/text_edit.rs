fn prev_char_boundary(text: &str, idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    text[..idx]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn next_char_boundary(text: &str, idx: usize) -> usize {
    if idx >= text.len() {
        return text.len();
    }
    let step = text[idx..]
        .chars()
        .next()
        .map(|ch| ch.len_utf8())
        .unwrap_or(0);
    (idx + step).min(text.len())
}

fn line_start(text: &str, idx: usize) -> usize {
    let idx = idx.min(text.len());
    text[..idx].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn line_end(text: &str, idx: usize) -> usize {
    let idx = idx.min(text.len());
    text[idx..]
        .find('\n')
        .map(|off| idx + off)
        .unwrap_or(text.len())
}

fn col_in_line(text: &str, idx: usize) -> usize {
    let start = line_start(text, idx);
    text[start..idx.min(text.len())].chars().count()
}

fn byte_index_for_col(text: &str, start: usize, target_col: usize) -> usize {
    let end = line_end(text, start);
    let mut col = 0usize;
    let mut last_idx = start;
    for (off, ch) in text[start..end].char_indices() {
        if col == target_col {
            return start + off;
        }
        col += 1;
        last_idx = start + off + ch.len_utf8();
    }
    if target_col == 0 {
        start
    } else {
        last_idx.min(end)
    }
}

pub fn line_col_at_cursor(text: &str, cursor: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut col = 0usize;
    for (idx, ch) in text.char_indices() {
        if idx >= cursor {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub fn move_cursor_left(text: &str, cursor: &mut usize) {
    *cursor = prev_char_boundary(text, (*cursor).min(text.len()));
}

pub fn move_cursor_right(text: &str, cursor: &mut usize) {
    *cursor = next_char_boundary(text, (*cursor).min(text.len()));
}

pub fn move_cursor_line_start(text: &str, cursor: &mut usize) {
    *cursor = line_start(text, (*cursor).min(text.len()));
}

pub fn move_cursor_line_end(text: &str, cursor: &mut usize) {
    *cursor = line_end(text, (*cursor).min(text.len()));
}

pub fn move_cursor_up(text: &str, cursor: &mut usize) {
    let cur = (*cursor).min(text.len());
    let cur_start = line_start(text, cur);
    if cur_start == 0 {
        return;
    }
    let target_col = col_in_line(text, cur);
    let prev_line_end = cur_start.saturating_sub(1);
    let prev_line_start = line_start(text, prev_line_end);
    *cursor = byte_index_for_col(text, prev_line_start, target_col);
}

pub fn move_cursor_down(text: &str, cursor: &mut usize) {
    let cur = (*cursor).min(text.len());
    let cur_end = line_end(text, cur);
    if cur_end >= text.len() {
        return;
    }
    let target_col = col_in_line(text, cur);
    let next_line_start = cur_end + 1;
    *cursor = byte_index_for_col(text, next_line_start, target_col);
}

pub fn insert_char_at_cursor(text: &mut String, cursor: &mut usize, ch: char) {
    let cur = (*cursor).min(text.len());
    text.insert(cur, ch);
    *cursor = cur + ch.len_utf8();
}

pub fn backspace_at_cursor(text: &mut String, cursor: &mut usize) {
    let cur = (*cursor).min(text.len());
    if cur == 0 {
        return;
    }
    let prev = prev_char_boundary(text, cur);
    text.drain(prev..cur);
    *cursor = prev;
}

pub fn delete_at_cursor(text: &mut String, cursor: &mut usize) {
    let cur = (*cursor).min(text.len());
    if cur >= text.len() {
        return;
    }
    let next = next_char_boundary(text, cur);
    text.drain(cur..next);
    *cursor = cur.min(text.len());
}
