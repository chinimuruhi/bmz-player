use super::*;

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
    score_graph_layout: bool,
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
        for line in lines {
            if self.handle_control(line) {
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
                } else if !(index == 985
                    && runtime_ops == [33]
                    && self.ops.get(&39).copied().unwrap_or(false))
                {
                    // WMII derives its judge-panel layout option 985 from either
                    // Score Graph=Off or autoplay. BMZ keeps LR2 autoplay as a
                    // runtime condition, but the skin settings must remain the
                    // authority for panel/graph visibility in both play modes.
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
            if self.active_score_graph_layout() && line.command.starts_with("DST_") {
                let mut line = line.clone();
                for index in 18..=20 {
                    if parse_i32(line.fields.get(index)) == 32
                        && let Some(field) = line.fields.get_mut(index)
                    {
                        field.clear();
                    }
                }
                builder.execute(&line)?;
            } else {
                builder.execute(line)?;
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
        match line.command.as_str() {
            "IF" => {
                let parent_active = self.active();
                let score_graph_layout = self.is_score_graph_layout_branch(line);
                let eval = self.eval_if(line);
                let condition = parent_active && eval.matches;
                self.stack.push(IfState {
                    parent_active,
                    branch_taken: condition,
                    active: condition,
                    score_graph_layout: condition && score_graph_layout,
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
                let score_graph_layout = self.is_score_graph_layout_branch(line);
                if !state.runtime_branches.is_empty() {
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
                state.score_graph_layout = state.active && score_graph_layout;
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
                    state.score_graph_layout = false;
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
        let score_graph_layout_branch = self.is_score_graph_layout_branch(line);
        let dependencies = &mut self.option_dependencies;
        let matches =
            line.fields.iter().skip(1).filter(|field| !field.trim().is_empty()).all(|field| {
                let option = parse_option_token(field);
                let option_id = option.abs();
                if option == 32 && score_graph_layout_branch {
                    // WMII wraps Score Graph=On layouts in OPTION_AUTOPLAYOFF.
                    // BMZ intentionally lets the selected Score Graph option
                    // control visibility even during autoplay.
                    true
                } else if let Some(alias) = self.runtime_aliases.get(&option_id) {
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

    fn is_score_graph_layout_branch(&self, line: &CsvLine) -> bool {
        self.ops.get(&39).copied().unwrap_or(false)
            && line
                .fields
                .iter()
                .skip(1)
                .map(|field| parse_option_token(field).abs())
                .any(|option| matches!(option, 983 | 984))
    }

    fn active_score_graph_layout(&self) -> bool {
        self.stack.iter().any(|state| state.active && state.score_graph_layout)
    }

    pub(super) fn active_runtime_ops(&self) -> Vec<i32> {
        self.stack
            .iter()
            .filter(|state| state.active)
            .flat_map(|state| state.runtime_ops.iter().copied())
            .collect()
    }
}
