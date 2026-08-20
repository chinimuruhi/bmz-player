import type { IrDoubleOption, IrRuleMode, LnScorePolicy } from '../../shared/types/ir'

export type RivalComparisonOutcome = 'win' | 'loss' | 'draw'

export interface RivalComparisonScoreRow {
  scoreId: string
  chartSha256: string
  chartMd5: string | null
  chartTitle: string
  chartSubtitle: string | null
  chartArtist: string | null
  chartMode: string
  chartLevel: number | null
  chartDifficulty: string | null
  lnPolicy: LnScorePolicy
  doubleOption: IrDoubleOption
  ruleMode: IrRuleMode
  scoring: string
  exScore: number
  minBp: number
}

export interface RivalComparisonRow {
  chart: {
    sha256: string
    md5: string | null
    title: string
    subtitle: string | null
    artist: string | null
    mode: string
    level: number | null
    difficulty: string | null
  }
  rule: {
    ln_policy: LnScorePolicy
    double_option: IrDoubleOption
    rule_mode: IrRuleMode
    scoring: string
  }
  self: { score_id: string; ex_score: number; min_bp: number }
  rival: { score_id: string; ex_score: number; min_bp: number }
  difference: { ex_score: number; min_bp: number }
  outcome: {
    ex_score: RivalComparisonOutcome
    min_bp: RivalComparisonOutcome
  }
}

export interface RivalComparisonSummary {
  ex_score: { wins: number; losses: number; draws: number }
  min_bp: { wins: number; losses: number; draws: number }
}

export interface RivalComparisonResult {
  summary: RivalComparisonSummary
  rows: RivalComparisonRow[]
}

export interface RivalMutation {
  targetPlayerId: string
  action: 'add' | 'remove'
}

export function parseRivalMutation(value: unknown, ownerPlayerId: string): RivalMutation {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('rival payload must be an object')
  }
  const body = value as Record<string, unknown>
  const targetPlayerId =
    typeof body.target_player_id === 'string' ? body.target_player_id.trim() : ''
  if (!targetPlayerId || targetPlayerId === ownerPlayerId) {
    throw new Error('valid target_player_id is required')
  }
  if (body.action !== 'add' && body.action !== 'remove') {
    throw new Error('action must be add or remove')
  }
  return { targetPlayerId, action: body.action }
}

export function buildRivalComparison(
  selfScores: readonly RivalComparisonScoreRow[],
  rivalScores: readonly RivalComparisonScoreRow[],
): RivalComparisonResult {
  const rivalByKey = new Map(rivalScores.map((score) => [scoreKey(score), score]))
  const summary: RivalComparisonSummary = {
    ex_score: { wins: 0, losses: 0, draws: 0 },
    min_bp: { wins: 0, losses: 0, draws: 0 },
  }
  const rows: RivalComparisonRow[] = []

  for (const selfScore of selfScores) {
    const rivalScore = rivalByKey.get(scoreKey(selfScore))
    if (!rivalScore) continue

    const exScoreDifference = selfScore.exScore - rivalScore.exScore
    const minBpDifference = selfScore.minBp - rivalScore.minBp
    const exScoreOutcome = higherWins(exScoreDifference)
    const minBpOutcome = lowerWins(minBpDifference)
    incrementSummary(summary.ex_score, exScoreOutcome)
    incrementSummary(summary.min_bp, minBpOutcome)

    rows.push({
      chart: {
        sha256: selfScore.chartSha256,
        md5: selfScore.chartMd5,
        title: selfScore.chartTitle,
        subtitle: selfScore.chartSubtitle,
        artist: selfScore.chartArtist,
        mode: selfScore.chartMode,
        level: selfScore.chartLevel,
        difficulty: selfScore.chartDifficulty,
      },
      rule: {
        ln_policy: selfScore.lnPolicy,
        double_option: selfScore.doubleOption,
        rule_mode: selfScore.ruleMode,
        scoring: selfScore.scoring,
      },
      self: {
        score_id: selfScore.scoreId,
        ex_score: selfScore.exScore,
        min_bp: selfScore.minBp,
      },
      rival: {
        score_id: rivalScore.scoreId,
        ex_score: rivalScore.exScore,
        min_bp: rivalScore.minBp,
      },
      difference: { ex_score: exScoreDifference, min_bp: minBpDifference },
      outcome: { ex_score: exScoreOutcome, min_bp: minBpOutcome },
    })
  }

  rows.sort(
    (left, right) =>
      compareLevelDescending(left.chart.level, right.chart.level) ||
      left.chart.title.localeCompare(right.chart.title) ||
      comparisonKey(left).localeCompare(comparisonKey(right)),
  )
  return { summary, rows }
}

function scoreKey(score: RivalComparisonScoreRow): string {
  return [
    score.chartSha256,
    score.lnPolicy,
    score.doubleOption,
    score.ruleMode,
    score.scoring,
  ].join('\0')
}

function comparisonKey(row: RivalComparisonRow): string {
  return [
    row.chart.sha256,
    row.rule.ln_policy,
    row.rule.double_option,
    row.rule.rule_mode,
    row.rule.scoring,
  ].join('\0')
}

function higherWins(difference: number): RivalComparisonOutcome {
  return difference > 0 ? 'win' : difference < 0 ? 'loss' : 'draw'
}

function lowerWins(difference: number): RivalComparisonOutcome {
  return difference < 0 ? 'win' : difference > 0 ? 'loss' : 'draw'
}

function incrementSummary(
  summary: { wins: number; losses: number; draws: number },
  outcome: RivalComparisonOutcome,
): void {
  if (outcome === 'win') summary.wins += 1
  else if (outcome === 'loss') summary.losses += 1
  else summary.draws += 1
}

function compareLevelDescending(left: number | null, right: number | null): number {
  if (left === null && right === null) return 0
  if (left === null) return 1
  if (right === null) return -1
  return right - left
}
