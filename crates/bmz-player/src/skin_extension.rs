use bmz_gameplay::rule::RuleMode;

use crate::ln_policy::{LnPolicySetting, LnScorePolicy};

pub(crate) const fn rule_mode_index(mode: RuleMode) -> usize {
    match mode {
        RuleMode::Beatoraja => 0,
        RuleMode::Lr2Oraja => 1,
        RuleMode::Dx => 2,
    }
}

pub(crate) const fn ln_policy_setting_index(policy: LnPolicySetting) -> usize {
    match policy {
        LnPolicySetting::AutoLn => 0,
        LnPolicySetting::AutoCn => 1,
        LnPolicySetting::AutoHcn => 2,
        LnPolicySetting::ForceLn => 3,
        LnPolicySetting::ForceCn => 4,
        LnPolicySetting::ForceHcn => 5,
    }
}

pub(crate) const fn ln_score_policy_index(policy: LnScorePolicy) -> usize {
    match policy {
        LnScorePolicy::AutoLn => 0,
        LnScorePolicy::AutoCn => 1,
        LnScorePolicy::AutoHcn => 2,
        LnScorePolicy::ForceLn => 3,
        LnScorePolicy::ForceCn => 4,
        LnScorePolicy::ForceHcn => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_indices_follow_the_documented_stable_order() {
        assert_eq!(rule_mode_index(RuleMode::Beatoraja), 0);
        assert_eq!(rule_mode_index(RuleMode::Lr2Oraja), 1);
        assert_eq!(rule_mode_index(RuleMode::Dx), 2);

        let score_policies = [
            LnScorePolicy::AutoLn,
            LnScorePolicy::AutoCn,
            LnScorePolicy::AutoHcn,
            LnScorePolicy::ForceLn,
            LnScorePolicy::ForceCn,
            LnScorePolicy::ForceHcn,
        ];
        for (index, (setting, score_policy)) in
            LnPolicySetting::ORDER.into_iter().zip(score_policies).enumerate()
        {
            assert_eq!(ln_policy_setting_index(setting), index);
            assert_eq!(ln_score_policy_index(score_policy), index);
        }
    }
}
