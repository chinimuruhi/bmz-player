use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum BeatorajaRandomControl<'a> {
    Random(&'a str),
    SetRandom(&'a str),
    If(&'a str),
    Else,
    ElseIf,
    EndIf,
    EndRandom,
}

pub(super) fn apply_beatoraja_random_control(
    text: &str,
    random_source: &BmsRandomSource,
    bms_random_choices: &mut Vec<i32>,
    warnings: &mut Vec<ImportWarning>,
) -> String {
    let mut rewritten = String::with_capacity(text.len());
    let mut rng = JavaRandom::new(random_source_seed(random_source) as i64);
    let mut choice_index = 0;
    let mut random_stack = Vec::new();
    let mut skip_stack = Vec::new();

    for (line_index, line) in text.lines().enumerate() {
        let line_number = line_index + 1;

        match beatoraja_random_control_line(line) {
            Some(BeatorajaRandomControl::Random(args)) => {
                if let Some(max) =
                    parse_beatoraja_control_int(args, line_number, "#RANDOM", warnings)
                {
                    let max = if max <= 0 {
                        warnings.push(ImportWarning::ParserDiagnostic {
                            code: "RandomZeroClamped".to_string(),
                            message: format!(
                                "line {line_number} #RANDOM {max} is treated as #RANDOM 1 for beatoraja compatibility"
                            ),
                        });
                        1
                    } else {
                        max
                    };
                    let selected = match random_source {
                        BmsRandomSource::Seed(_) => rng.next_int_bound(max) + 1,
                        BmsRandomSource::Choices(choices) => {
                            let current_index = choice_index;
                            choice_index += 1;
                            match choices.get(current_index).copied() {
                                None => {
                                    warnings.push(ImportWarning::ParserDiagnostic {
                                        code: "BmsRandomChoiceMissing".to_string(),
                                        message: format!(
                                            "line {line_number} #RANDOM {max} has no recorded choice at index {current_index}; using deterministic fallback"
                                        ),
                                    });
                                    rng.next_int_bound(max) + 1
                                }
                                Some(choice) if !(1..=max).contains(&choice) => {
                                    let clamped = choice.clamp(1, max);
                                    warnings.push(ImportWarning::ParserDiagnostic {
                                        code: "BmsRandomChoiceOutOfRange".to_string(),
                                        message: format!(
                                            "line {line_number} #RANDOM {max} cannot use recorded choice {choice} at index {current_index}; clamping to {clamped}"
                                        ),
                                    });
                                    clamped
                                }
                                Some(choice) => choice,
                            }
                        }
                    };
                    bms_random_choices.push(selected);
                    random_stack.push(selected);
                }
            }
            Some(BeatorajaRandomControl::SetRandom(args)) => {
                if let Some(selected) =
                    parse_beatoraja_control_int(args, line_number, "#SETRANDOM", warnings)
                {
                    random_stack.push(selected);
                }
            }
            Some(BeatorajaRandomControl::If(args)) => {
                if let Some(&selected) = random_stack.last() {
                    if let Some(condition) =
                        parse_beatoraja_control_int(args, line_number, "#IF", warnings)
                    {
                        skip_stack.push(selected != condition);
                    }
                } else {
                    warnings.push(ImportWarning::ParserDiagnostic {
                        code: "BeatorajaRandomIfWithoutRandom".to_string(),
                        message: format!(
                            "line {line_number} #IF has no active #RANDOM; continuing like beatoraja"
                        ),
                    });
                }
            }
            Some(BeatorajaRandomControl::Else | BeatorajaRandomControl::ElseIf) => {
                // beatoraja (jbms-parser BMSDecoder) は予約語として RANDOM / IF /
                // ENDIF / ENDRANDOM しか扱わず、#ELSE / #ELSEIF 行そのものを
                // 無視する。つまり直前の #IF の skip 状態がそのまま継続する
                // (#IF 一致時は #ELSE 側のブロックも取り込まれる)。BMZ も同じ
                // 実行結果になるよう、行を落とすだけで skip 状態は変更しない。
                warnings.push(ImportWarning::ParserDiagnostic {
                    code: "BeatorajaRandomUnsupportedElse".to_string(),
                    message: format!(
                        "line {line_number} random #ELSE/#ELSEIF is ignored for beatoraja compatibility"
                    ),
                });
            }
            Some(BeatorajaRandomControl::EndIf) => {
                if skip_stack.pop().is_none() {
                    warnings.push(ImportWarning::ParserDiagnostic {
                        code: "BeatorajaRandomEndifWithoutIf".to_string(),
                        message: format!(
                            "line {line_number} #ENDIF has no active #IF; continuing like beatoraja"
                        ),
                    });
                }
            }
            Some(BeatorajaRandomControl::EndRandom) => {
                if random_stack.pop().is_none() {
                    warnings.push(ImportWarning::ParserDiagnostic {
                        code: "BeatorajaRandomEndrandomWithoutRandom".to_string(),
                        message: format!(
                            "line {line_number} #ENDRANDOM has no active #RANDOM; continuing like beatoraja"
                        ),
                    });
                }
            }
            None => {
                if let Some(command) = bms_rs_random_typo_control_line(line) {
                    warnings.push(ImportWarning::ParserDiagnostic {
                        code: "BeatorajaRandomIgnoredTypoControl".to_string(),
                        message: format!(
                            "line {line_number} {command} is ignored for beatoraja compatibility"
                        ),
                    });
                } else if !skip_stack.last().copied().unwrap_or(false) {
                    rewritten.push_str(line);
                }
            }
        }

        rewritten.push('\n');
    }

    if let BmsRandomSource::Choices(choices) = random_source
        && choice_index < choices.len()
    {
        warnings.push(ImportWarning::ParserDiagnostic {
            code: "BmsRandomChoiceExtra".to_string(),
            message: format!(
                "{} recorded BMS #RANDOM choice(s) were unused",
                choices.len() - choice_index
            ),
        });
    }

    rewritten
}

pub(super) fn random_source_seed(random_source: &BmsRandomSource) -> u64 {
    match random_source {
        BmsRandomSource::Seed(seed) => seed.unwrap_or(0),
        BmsRandomSource::Choices(_) => 0,
    }
}

pub(super) fn beatoraja_random_control_line(line: &str) -> Option<BeatorajaRandomControl<'_>> {
    let body = line.trim_start().strip_prefix('#')?;
    if starts_ignore_ascii_case(body, "ENDRANDOM") {
        return Some(BeatorajaRandomControl::EndRandom);
    }
    if starts_ignore_ascii_case(body, "ENDIF") {
        return Some(BeatorajaRandomControl::EndIf);
    }
    if starts_ignore_ascii_case(body, "ELSEIF") {
        return Some(BeatorajaRandomControl::ElseIf);
    }
    if starts_ignore_ascii_case(body, "ELSE") {
        return Some(BeatorajaRandomControl::Else);
    }
    if starts_ignore_ascii_case(body, "SETRANDOM") {
        return Some(BeatorajaRandomControl::SetRandom(command_args(body, "SETRANDOM")));
    }
    if starts_ignore_ascii_case(body, "RANDOM") {
        return Some(BeatorajaRandomControl::Random(command_args(body, "RANDOM")));
    }
    if starts_ignore_ascii_case(body, "IF") {
        return Some(BeatorajaRandomControl::If(command_args(body, "IF")));
    }
    None
}

pub(super) fn bms_rs_random_typo_control_line(line: &str) -> Option<&'static str> {
    let body = line.trim_start().strip_prefix('#')?.trim();
    if body.eq_ignore_ascii_case("END IF") {
        return Some("#END IF");
    }
    if body.eq_ignore_ascii_case("END RANDOM") {
        return Some("#END RANDOM");
    }
    None
}

pub(super) fn starts_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value.get(..prefix.len()).is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

pub(super) fn command_args<'a>(body: &'a str, command: &str) -> &'a str {
    body.get(command.len() + 1..).unwrap_or("").trim()
}

pub(super) fn strip_lnobj_commands(text: &str) -> String {
    let mut rewritten = String::with_capacity(text.len());
    for line in text.lines() {
        if lnobj_command_args(line).is_none() {
            rewritten.push_str(line);
        }
        rewritten.push('\n');
    }
    rewritten
}

pub(super) fn strip_empty_metadata_commands(text: &str) -> String {
    let mut rewritten = String::with_capacity(text.len());
    for line in text.lines() {
        if !is_empty_metadata_command(line) {
            rewritten.push_str(line);
        }
        rewritten.push('\n');
    }
    rewritten
}

pub(super) fn is_empty_metadata_command(line: &str) -> bool {
    let Some(body) = line.trim().strip_prefix('#') else {
        return false;
    };
    let (name, value) = split_bms_header_command(body.trim_start());
    value.is_empty()
        && matches!(
            name.to_ascii_uppercase().as_str(),
            "PLAYER"
                | "GENRE"
                | "TITLE"
                | "SUBTITLE"
                | "ARTIST"
                | "SUBARTIST"
                | "BPM"
                | "PLAYLEVEL"
                | "DIFFICULTY"
                | "RANK"
                | "DEFEXRANK"
                | "TOTAL"
                | "STAGEFILE"
                | "BANNER"
                | "BACKBMP"
                | "PREVIEW"
                | "VOLWAV"
                | "LNTYPE"
                | "LNMODE"
        )
}

pub(super) fn extract_lnobj_wav_key(
    text: &str,
    base62_obj_ids: bool,
    warnings: &mut Vec<ImportWarning>,
) -> Option<u16> {
    let mut lnobj_wav_key = None;
    for (line_index, line) in text.lines().enumerate() {
        let Some(args) = lnobj_command_args(line) else {
            continue;
        };
        let line_number = line_index + 1;
        let Some(token) = args.split_whitespace().next() else {
            warnings.push(ImportWarning::ParserDiagnostic {
                code: "InvalidLnobj".to_string(),
                message: format!("line {line_number} #LNOBJ has no object id"),
            });
            continue;
        };
        match ObjId::try_from(token, base62_obj_ids) {
            Ok(obj_id) => lnobj_wav_key = Some(obj_id.as_u16()),
            Err(err) => warnings.push(ImportWarning::ParserDiagnostic {
                code: "InvalidLnobj".to_string(),
                message: format!(
                    "line {line_number} #LNOBJ has invalid object id {token:?}: {err}"
                ),
            }),
        }
    }
    lnobj_wav_key
}

pub(super) fn lnobj_command_args(line: &str) -> Option<&str> {
    let body = line.trim_start().strip_prefix('#')?.trim_start();
    let (name, value) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
    name.eq_ignore_ascii_case("LNOBJ").then_some(value.trim())
}

pub(super) fn parse_beatoraja_control_int(
    args: &str,
    line_number: usize,
    command: &str,
    warnings: &mut Vec<ImportWarning>,
) -> Option<i32> {
    match args.parse::<i32>() {
        Ok(value) => Some(value),
        Err(_) => {
            warnings.push(ImportWarning::ParserDiagnostic {
                code: "BeatorajaRandomInvalidArgument".to_string(),
                message: format!(
                    "line {line_number} {command} has invalid integer argument {args:?}; continuing like beatoraja"
                ),
            });
            None
        }
    }
}
