use super::*;

pub(in crate::skin) fn skin_state_number_expr_term(
    term: &str,
    state: &SkinDrawState,
) -> Option<i64> {
    if let Some(ref_id) = parse_skin_number_operand(term) {
        return skin_state_number(ref_id, state);
    }
    if let Some((coefficient, operand)) = term.split_once('*') {
        let coefficient = coefficient.parse::<i64>().ok()?;
        let ref_id = parse_skin_number_operand(operand.trim())?;
        return skin_state_number(ref_id, state).map(|value| coefficient * value);
    }
    term.parse::<i64>().ok()
}

pub(in crate::skin) fn skin_state_float_expr(expr: &str, state: &SkinDrawState) -> Option<f32> {
    let expr = strip_wrapping_parentheses(expr.trim());
    if expr.is_empty() {
        return None;
    }
    if let Some(inner) = expr.strip_prefix("floor(").and_then(|value| value.strip_suffix(')')) {
        return skin_state_float_expr(inner.trim(), state).map(f32::floor);
    }
    if let Some(inner) = expr.strip_prefix("max(0,").and_then(|value| value.strip_suffix(')')) {
        return skin_state_float_expr(inner.trim(), state).map(|value| value.max(0.0));
    }
    skin_state_additive_float_expr(expr, state)
}

pub(in crate::skin) fn strip_wrapping_parentheses(mut expr: &str) -> &str {
    loop {
        let trimmed = expr.trim();
        if !outer_parentheses_wrap_expression(trimmed) {
            return trimmed;
        }
        expr = &trimmed[1..trimmed.len() - 1];
    }
}

pub(in crate::skin) fn outer_parentheses_wrap_expression(expr: &str) -> bool {
    if !expr.starts_with('(') || !expr.ends_with(')') {
        return false;
    }
    let mut depth = 0_i32;
    let last_index = expr.len() - 1;
    for (index, ch) in expr.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && index < last_index {
                    return false;
                }
            }
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

pub(in crate::skin) fn skin_state_additive_float_expr(
    expr: &str,
    state: &SkinDrawState,
) -> Option<f32> {
    let mut depth = 0_i32;
    let mut sign = 1.0_f32;
    let mut start = 0_usize;
    let mut total = 0.0_f32;

    for (index, ch) in expr.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '+' | '-' if depth == 0 => {
                let term = expr[start..index].trim();
                if term.is_empty() {
                    sign = if ch == '-' { -1.0 } else { 1.0 };
                    start = index + ch.len_utf8();
                    continue;
                }
                total += sign * skin_state_float_mul_div_expr(term, state)?;
                sign = if ch == '-' { -1.0 } else { 1.0 };
                start = index + ch.len_utf8();
            }
            _ => {}
        }
        if depth < 0 {
            return None;
        }
    }
    if depth != 0 {
        return None;
    }
    let term = expr[start..].trim();
    if term.is_empty() {
        return None;
    }
    total += sign * skin_state_float_mul_div_expr(term, state)?;
    Some(total)
}

pub(in crate::skin) fn skin_state_float_mul_div_expr(
    expr: &str,
    state: &SkinDrawState,
) -> Option<f32> {
    let mut depth = 0_i32;
    let mut start = 0_usize;
    let mut value: Option<f32> = None;
    let mut operator = '*';

    for (index, ch) in expr.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '*' | '/' if depth == 0 => {
                let factor = skin_state_float_expr_term(expr[start..index].trim(), state)?;
                value = Some(apply_float_mul_div(value, factor, operator));
                operator = ch;
                start = index + ch.len_utf8();
            }
            _ => {}
        }
        if depth < 0 {
            return None;
        }
    }
    if depth != 0 {
        return None;
    }

    let factor = skin_state_float_expr_term(expr[start..].trim(), state)?;
    Some(apply_float_mul_div(value, factor, operator))
}

pub(in crate::skin) fn apply_float_mul_div(
    current: Option<f32>,
    factor: f32,
    operator: char,
) -> f32 {
    let Some(current) = current else { return factor };
    match operator {
        '*' => current * factor,
        '/' if factor.abs() < f32::EPSILON => 0.0,
        '/' => current / factor,
        _ => current,
    }
}

pub(in crate::skin) fn skin_state_float_expr_term(
    term: &str,
    state: &SkinDrawState,
) -> Option<f32> {
    let term = term.trim();
    let stripped = strip_wrapping_parentheses(term);
    if stripped.len() != term.len() {
        return skin_state_float_expr(stripped, state);
    }
    if term.starts_with("floor(") || term.starts_with("max(0,") {
        return skin_state_float_expr(term, state);
    }
    if let Some(ref_id) = parse_skin_float_number_operand(term) {
        return skin_state_float_number(ref_id, state);
    }
    if let Some(event_id) = parse_skin_event_index_operand(term) {
        return Some(skin_state_event_index(event_id, state) as f32);
    }
    if let Some(ref_id) = parse_skin_number_operand(term) {
        return skin_state_number(ref_id, state).map(|value| value as f32);
    }
    if let Some(option_id) = parse_skin_option_operand(term) {
        return Some(if test_skin_op(option_id, &[], state) { 1.0 } else { 0.0 });
    }
    term.parse::<f32>().ok()
}
