use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CourseDefinition {
    pub key: String,
    pub title: String,
    pub kind: CourseKind,
    pub entries: Vec<CourseEntry>,
    pub constraints: CourseConstraints,
    pub trophies: Vec<CourseTrophy>,
    pub release: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CourseKind {
    Course,
    Dan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseEntry {
    pub title_hint: String,
    pub md5: Option<String>,
    pub sha256: Option<String>,
    pub chart_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseConstraints {
    pub class: CourseClassConstraint,
    pub speed: CourseSpeedConstraint,
    pub judge: CourseJudgeConstraint,
    pub gauge: CourseGaugeConstraint,
    pub ln: CourseLnConstraint,
    pub source_constraints: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CourseClassConstraint {
    None,
    Grade,
    GradeMirrorAllowed,
    GradeRandomAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CourseSpeedConstraint {
    #[default]
    Free,
    NoSpeed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CourseJudgeConstraint {
    #[default]
    Normal,
    NoGood,
    NoGreat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CourseGaugeConstraint {
    Default,
    Lr2,
    Keys5,
    Keys7,
    Keys9,
    Keys24,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CourseLnConstraint {
    Default,
    Ln,
    Cn,
    Hcn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CourseTrophy {
    pub name: String,
    pub max_miss_rate: f32,
    pub min_score_rate: f32,
}

impl Default for CourseConstraints {
    fn default() -> Self {
        Self {
            class: CourseClassConstraint::None,
            speed: CourseSpeedConstraint::Free,
            judge: CourseJudgeConstraint::Normal,
            gauge: CourseGaugeConstraint::Default,
            ln: CourseLnConstraint::Default,
            source_constraints: Vec::new(),
        }
    }
}

impl CourseConstraints {
    pub fn from_beatoraja_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        let mut constraints = Self::default();
        for name in names {
            constraints.source_constraints.push(name.to_string());
            match name {
                "grade" | "CLASS" => constraints.class = CourseClassConstraint::Grade,
                "grade_mirror" | "MIRROR" => {
                    constraints.class = CourseClassConstraint::GradeMirrorAllowed;
                }
                "grade_random" | "RANDOM" => {
                    constraints.class = CourseClassConstraint::GradeRandomAllowed;
                }
                "no_speed" | "NO_SPEED" => constraints.speed = CourseSpeedConstraint::NoSpeed,
                "no_good" | "NO_GOOD" => constraints.judge = CourseJudgeConstraint::NoGood,
                "no_great" | "NO_GREAT" => constraints.judge = CourseJudgeConstraint::NoGreat,
                "gauge_lr2" | "GAUGE_LR2" => constraints.gauge = CourseGaugeConstraint::Lr2,
                "gauge_5k" | "GAUGE_5KEYS" => constraints.gauge = CourseGaugeConstraint::Keys5,
                "gauge_7k" | "GAUGE_7KEYS" => constraints.gauge = CourseGaugeConstraint::Keys7,
                "gauge_9k" | "GAUGE_9KEYS" => constraints.gauge = CourseGaugeConstraint::Keys9,
                "gauge_24k" | "GAUGE_24KEYS" => {
                    constraints.gauge = CourseGaugeConstraint::Keys24;
                }
                "ln" | "LN" if constraints.ln == CourseLnConstraint::Default => {
                    constraints.ln = CourseLnConstraint::Ln;
                }
                "cn" | "CN" if constraints.ln == CourseLnConstraint::Default => {
                    constraints.ln = CourseLnConstraint::Cn;
                }
                "hcn" | "HCN" if constraints.ln == CourseLnConstraint::Default => {
                    constraints.ln = CourseLnConstraint::Hcn;
                }
                _ => {}
            }
        }
        constraints
    }

    /// Difficulty-table/IR-compatible canonical constraint names.
    pub fn canonical_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        match self.class {
            CourseClassConstraint::None => {}
            CourseClassConstraint::Grade => names.push("grade"),
            CourseClassConstraint::GradeMirrorAllowed => names.push("grade_mirror"),
            CourseClassConstraint::GradeRandomAllowed => names.push("grade_random"),
        }
        if self.speed == CourseSpeedConstraint::NoSpeed {
            names.push("no_speed");
        }
        match self.judge {
            CourseJudgeConstraint::Normal => {}
            CourseJudgeConstraint::NoGood => names.push("no_good"),
            CourseJudgeConstraint::NoGreat => names.push("no_great"),
        }
        match self.gauge {
            CourseGaugeConstraint::Default => {}
            CourseGaugeConstraint::Lr2 => names.push("gauge_lr2"),
            CourseGaugeConstraint::Keys5 => names.push("gauge_5k"),
            CourseGaugeConstraint::Keys7 => names.push("gauge_7k"),
            CourseGaugeConstraint::Keys9 => names.push("gauge_9k"),
            CourseGaugeConstraint::Keys24 => names.push("gauge_24k"),
        }
        match self.ln {
            CourseLnConstraint::Default => {}
            CourseLnConstraint::Ln => names.push("ln"),
            CourseLnConstraint::Cn => names.push("cn"),
            CourseLnConstraint::Hcn => names.push("hcn"),
        }
        names
    }

    pub fn is_dan(&self) -> bool {
        self.class != CourseClassConstraint::None
    }
}

impl CourseDefinition {
    pub fn derive_kind_from_constraints(constraints: &CourseConstraints) -> CourseKind {
        if constraints.is_dan() { CourseKind::Dan } else { CourseKind::Course }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beatoraja_constraint_names_are_normalized_by_category() {
        let constraints = CourseConstraints::from_beatoraja_names([
            "grade_mirror",
            "no_speed",
            "no_good",
            "gauge_7k",
            "cn",
        ]);

        assert_eq!(constraints.class, CourseClassConstraint::GradeMirrorAllowed);
        assert_eq!(constraints.speed, CourseSpeedConstraint::NoSpeed);
        assert_eq!(constraints.judge, CourseJudgeConstraint::NoGood);
        assert_eq!(constraints.gauge, CourseGaugeConstraint::Keys7);
        assert_eq!(constraints.ln, CourseLnConstraint::Cn);
        assert_eq!(constraints.source_constraints.len(), 5);
        assert!(constraints.is_dan());
    }

    #[test]
    fn first_beatoraja_ln_constraint_wins() {
        let constraints = CourseConstraints::from_beatoraja_names(["cn", "hcn", "ln"]);

        assert_eq!(constraints.ln, CourseLnConstraint::Cn);
        assert_eq!(constraints.source_constraints, ["cn", "hcn", "ln"]);
    }

    #[test]
    fn beatoraja_local_enum_names_are_supported() {
        let constraints = CourseConstraints::from_beatoraja_names([
            "RANDOM",
            "NO_SPEED",
            "NO_GREAT",
            "GAUGE_24KEYS",
            "HCN",
        ]);

        assert_eq!(constraints.class, CourseClassConstraint::GradeRandomAllowed);
        assert_eq!(constraints.speed, CourseSpeedConstraint::NoSpeed);
        assert_eq!(constraints.judge, CourseJudgeConstraint::NoGreat);
        assert_eq!(constraints.gauge, CourseGaugeConstraint::Keys24);
        assert_eq!(constraints.ln, CourseLnConstraint::Hcn);
        assert_eq!(
            constraints.canonical_names(),
            ["grade_random", "no_speed", "no_great", "gauge_24k", "hcn"]
        );
    }
}
