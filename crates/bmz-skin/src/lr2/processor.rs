use super::*;

const OPTION_AUTOPLAYOFF: i32 = 32;
const OPTION_AUTOPLAYON: i32 = 33;
const OPTION_SCOREGRAPH: i32 = 39;

pub(super) struct Processor {
    pub(super) ops: HashMap<i32, bool>,
    runtime_aliases: HashMap<i32, Vec<i32>>,
    stack: Vec<IfState>,
    pub(super) option_dependencies: BTreeMap<i32, bool>,
}

#[derive(Debug, Clone)]
struct IfState {
    parent_active: bool,
    branch_taken: bool,
    active: bool,
    prefer_load_time_branch: bool,
    runtime_ops: Vec<i32>,
    runtime_branches: Vec<Vec<i32>>,
}

impl Processor {
    pub(super) fn new(ops: HashMap<i32, bool>) -> Self {
        Self {
            ops,
            runtime_aliases: HashMap::new(),
            stack: Vec::new(),
            option_dependencies: BTreeMap::new(),
        }
    }

    pub(super) fn process_lines(
        &mut self,
        lines: &[CsvLine],
        current_path: &Path,
        builder: &mut CsvBuilder,
    ) -> Result<()> {
        let mut has_selected_score_graph_layout = false;
        for (index, line) in lines.iter().enumerate() {
            let branch_lines = &lines[index.saturating_add(1)..];
            if !has_selected_score_graph_layout {
                has_selected_score_graph_layout = self.has_selected_score_graph_layout(lines);
            }
            let prefer_load_time_else_if =
                line.command == "IF" && self.has_matching_load_time_else_if(branch_lines);
            let promote_score_graph_layout = has_selected_score_graph_layout
                && self.is_selected_score_graph_layout_branch(line, branch_lines);
            let suppress_autoplay_bga_layout = has_selected_score_graph_layout
                && self.is_autoplay_bga_layout_branch(line, branch_lines);
            if self.handle_control_with_preference(
                line,
                prefer_load_time_else_if,
                promote_score_graph_layout,
                suppress_autoplay_bga_layout,
            ) {
                continue;
            }
            if !self.active() {
                continue;
            }
            if line.command == "SETOPTION" {
                let index = parse_i32(line.fields.get(1));
                let value = parse_i32(line.fields.get(2)) >= 1;
                let runtime_ops = self.active_runtime_ops();
                if runtime_ops.is_empty() {
                    self.ops.insert(index, value);
                    builder.header.selected_ops.insert(index, value);
                } else {
                    if value && !self.ops.get(&index.abs()).copied().unwrap_or(false) {
                        self.runtime_aliases.entry(index.abs()).or_insert(runtime_ops.clone());
                    }
                    builder.register_runtime_option_alias(index, value, &runtime_ops);
                }
                continue;
            }
            builder.conditional_ops = self.active_runtime_ops();
            builder.apply_play_header_command(line);
            if line.command == "INCLUDE" {
                let include = resolve_include_path(builder, current_path, field(line, 1));
                if include.is_file() {
                    builder.record_loaded_file_dependency(&include);
                    let include_lines = read_csv_lines(&include)?;
                    self.process_lines(&include_lines, &include, builder)?;
                } else {
                    builder.warn(format!("lr2 include not found: {}", include.display()));
                }
                continue;
            }
            builder.execute(line)?;
            if self.is_score_graph_destination(line) {
                // LR2 play skins commonly guard score-graph destinations with
                // OPTION_AUTOPLAYOFF in addition to OPTION_SCOREGRAPH. BMZ lets
                // the configured Score Graph option control those destinations
                // in autoplay, while preserving op32 on the surrounding BGA and
                // song-information layout.
                builder.remove_option_from_current_destination(OPTION_AUTOPLAYOFF);
            }
        }
        builder.conditional_ops.clear();
        Ok(())
    }

    pub(super) fn should_execute(&mut self, line: &CsvLine) -> bool {
        if self.handle_control(line) {
            return false;
        }
        self.active()
    }

    pub(super) fn handle_control(&mut self, line: &CsvLine) -> bool {
        self.handle_control_with_preference(line, false, false, false)
    }

    fn handle_control_with_preference(
        &mut self,
        line: &CsvLine,
        prefer_load_time_else_if: bool,
        promote_score_graph_layout: bool,
        suppress_autoplay_bga_layout: bool,
    ) -> bool {
        match line.command.as_str() {
            "IF" => {
                let parent_active = self.active();
                let mut eval = self.eval_if(line);
                if promote_score_graph_layout {
                    eval.runtime_ops.retain(|option| option.abs() != OPTION_AUTOPLAYOFF);
                }
                let condition = parent_active
                    && eval.matches
                    && !suppress_autoplay_bga_layout
                    && (!prefer_load_time_else_if || eval.runtime_ops.is_empty());
                self.stack.push(IfState {
                    parent_active,
                    branch_taken: condition,
                    active: condition,
                    prefer_load_time_branch: prefer_load_time_else_if,
                    runtime_ops: if condition { eval.runtime_ops.clone() } else { Vec::new() },
                    runtime_branches: if condition && !eval.runtime_ops.is_empty() {
                        vec![eval.runtime_ops]
                    } else {
                        Vec::new()
                    },
                });
                true
            }
            "ELSEIF" => {
                let Some(mut state) = self.stack.pop() else {
                    return true;
                };
                if state.prefer_load_time_branch && !state.branch_taken {
                    let eval = self.eval_if(line);
                    state.active =
                        state.parent_active && eval.matches && eval.runtime_ops.is_empty();
                    state.branch_taken |= state.active;
                    state.runtime_ops.clear();
                } else if !state.runtime_branches.is_empty() {
                    let eval = self.eval_if(line);
                    if !state.parent_active || !eval.matches || eval.runtime_ops.is_empty() {
                        state.active = false;
                        state.runtime_ops.clear();
                    } else if let Some(mut previous) =
                        negate_runtime_branches(&state.runtime_branches)
                    {
                        previous.extend(eval.runtime_ops.iter().copied());
                        state.active = true;
                        state.runtime_ops = previous;
                        state.runtime_branches.push(eval.runtime_ops);
                    } else {
                        state.active = false;
                        state.runtime_ops.clear();
                    }
                } else if !state.parent_active || state.branch_taken {
                    state.active = false;
                    state.runtime_ops.clear();
                } else {
                    let eval = self.eval_if(line);
                    state.active = eval.matches;
                    state.branch_taken |= state.active;
                    state.runtime_ops = if state.active { eval.runtime_ops } else { Vec::new() };
                }
                self.stack.push(state);
                true
            }
            "ELSE" => {
                if let Some(state) = self.stack.last_mut() {
                    if !state.runtime_branches.is_empty() {
                        if let Some(ops) = negate_runtime_branches(&state.runtime_branches) {
                            state.active = state.parent_active;
                            state.runtime_ops = ops;
                        } else {
                            state.active = false;
                            state.runtime_ops.clear();
                        }
                    } else {
                        state.active = state.parent_active && !state.branch_taken;
                        state.runtime_ops.clear();
                    }
                    state.branch_taken = true;
                }
                true
            }
            "ENDIF" => {
                self.stack.pop();
                true
            }
            _ => false,
        }
    }

    pub(super) fn active(&self) -> bool {
        self.stack.iter().all(|state| state.active)
    }

    pub(super) fn eval_if(&mut self, line: &CsvLine) -> IfEval {
        let mut runtime_ops = Vec::new();
        let ops = self.ops.clone();
        let dependencies = &mut self.option_dependencies;
        let matches =
            line.fields.iter().skip(1).filter(|field| !field.trim().is_empty()).all(|field| {
                let option = parse_option_token(field);
                let option_id = option.abs();
                if let Some(alias) = self.runtime_aliases.get(&option_id) {
                    if option >= 0 {
                        runtime_ops.extend(alias.iter().copied());
                        true
                    } else if alias.len() == 1 {
                        runtime_ops.push(-alias[0]);
                        true
                    } else {
                        false
                    }
                } else if is_runtime_lr2_option(option_id) {
                    runtime_ops.push(option);
                    true
                } else if let Some(enabled) = ops.get(&option_id).copied() {
                    dependencies.insert(option_id, enabled);
                    if option >= 0 { enabled } else { !enabled }
                } else {
                    dependencies.insert(option_id, false);
                    option < 0
                }
            });
        IfEval { matches, runtime_ops }
    }

    fn is_score_graph_destination(&self, line: &CsvLine) -> bool {
        self.ops.get(&OPTION_SCOREGRAPH).copied().unwrap_or(false)
            && destination_has_option(line, OPTION_SCOREGRAPH)
    }

    fn has_selected_score_graph_layout(&self, lines: &[CsvLine]) -> bool {
        lines.iter().enumerate().any(|(index, line)| {
            self.is_selected_score_graph_layout_branch(line, &lines[index.saturating_add(1)..])
        })
    }

    fn is_selected_score_graph_layout_branch(
        &self,
        line: &CsvLine,
        branch_lines: &[CsvLine],
    ) -> bool {
        self.ops.get(&OPTION_SCOREGRAPH).copied().unwrap_or(false)
            && line.command == "IF"
            && condition_has_option(line, OPTION_AUTOPLAYOFF)
            && self.condition_matches_ignoring(line, OPTION_AUTOPLAYOFF)
            && branch_contains(branch_lines, |line| line.command == "SRC_BGA")
            && branch_contains(branch_lines, |line| destination_has_option(line, OPTION_SCOREGRAPH))
    }

    fn is_autoplay_bga_layout_branch(&self, line: &CsvLine, branch_lines: &[CsvLine]) -> bool {
        line.command == "IF"
            && condition_has_option(line, OPTION_AUTOPLAYON)
            && self.condition_matches_ignoring(line, OPTION_AUTOPLAYON)
            && branch_contains(branch_lines, |line| line.command == "SRC_BGA")
    }

    fn condition_matches_ignoring(&self, line: &CsvLine, ignored_option: i32) -> bool {
        line.fields.iter().skip(1).filter(|field| !field.trim().is_empty()).all(|field| {
            let option = parse_option_token(field);
            let option_id = option.abs();
            if option == ignored_option {
                true
            } else if self.runtime_aliases.contains_key(&option_id)
                || is_runtime_lr2_option(option_id)
            {
                false
            } else {
                let enabled = self.ops.get(&option_id).copied().unwrap_or(false);
                if option >= 0 { enabled } else { !enabled }
            }
        })
    }

    fn has_matching_load_time_else_if(&self, lines: &[CsvLine]) -> bool {
        let mut depth = 0usize;
        for line in lines {
            match line.command.as_str() {
                "IF" => depth = depth.saturating_add(1),
                "ENDIF" if depth == 0 => break,
                "ENDIF" => depth = depth.saturating_sub(1),
                "ELSEIF" if depth == 0 => {
                    if self.eval_load_time_if(line) == Some(true) {
                        return true;
                    }
                }
                "ELSE" if depth == 0 => break,
                _ => {}
            }
        }
        false
    }

    fn eval_load_time_if(&self, line: &CsvLine) -> Option<bool> {
        let mut matches = true;
        for field in line.fields.iter().skip(1).filter(|field| !field.trim().is_empty()) {
            let option = parse_option_token(field);
            let option_id = option.abs();
            if self.runtime_aliases.contains_key(&option_id) || is_runtime_lr2_option(option_id) {
                return None;
            }
            let enabled = self.ops.get(&option_id).copied().unwrap_or(false);
            matches &= if option >= 0 { enabled } else { !enabled };
        }
        Some(matches)
    }

    pub(super) fn active_runtime_ops(&self) -> Vec<i32> {
        self.stack
            .iter()
            .filter(|state| state.active)
            .flat_map(|state| state.runtime_ops.iter().copied())
            .collect()
    }
}

fn condition_has_option(line: &CsvLine, option: i32) -> bool {
    line.fields
        .iter()
        .skip(1)
        .filter(|field| !field.trim().is_empty())
        .any(|field| parse_option_token(field) == option)
}

fn destination_has_option(line: &CsvLine, option: i32) -> bool {
    line.command.starts_with("DST_")
        && line.fields.iter().skip(18).take(3).any(|field| parse_option_token(field) == option)
}

fn branch_contains(lines: &[CsvLine], predicate: impl Fn(&CsvLine) -> bool) -> bool {
    let mut depth = 0usize;
    for line in lines {
        match line.command.as_str() {
            "IF" => depth = depth.saturating_add(1),
            "ENDIF" if depth == 0 => break,
            "ENDIF" => depth = depth.saturating_sub(1),
            "ELSEIF" | "ELSE" if depth == 0 => break,
            _ if predicate(line) => return true,
            _ => {}
        }
    }
    false
}
