import { describe, expect, test } from 'bun:test'
import {
  buildRivalComparison,
  parseRivalMutation,
  type RivalComparisonScoreRow,
} from './rivals_core'

describe('rival mutation input', () => {
  test('accepts explicit add and remove actions', () => {
    expect(parseRivalMutation({ target_player_id: ' rival ', action: 'add' }, 'self')).toEqual({
      targetPlayerId: 'rival',
      action: 'add',
    })
    expect(parseRivalMutation({ target_player_id: 'rival', action: 'remove' }, 'self')).toEqual({
      targetPlayerId: 'rival',
      action: 'remove',
    })
  })

  test('rejects self rivals and implicit actions', () => {
    expect(() => parseRivalMutation({ target_player_id: 'self', action: 'add' }, 'self')).toThrow(
      'valid target_player_id is required',
    )
    expect(() => parseRivalMutation({ target_player_id: 'rival' }, 'self')).toThrow(
      'action must be add or remove',
    )
  })
})

describe('rival comparison', () => {
  test('counts EX and BP results with their respective directions', () => {
    const result = buildRivalComparison(
      [score({ scoreId: 'self-a', chartSha256: 'a', exScore: 1800, minBp: 20 })],
      [score({ scoreId: 'rival-a', chartSha256: 'a', exScore: 1700, minBp: 25 })],
    )

    expect(result.summary).toEqual({
      ex_score: { wins: 1, losses: 0, draws: 0 },
      min_bp: { wins: 1, losses: 0, draws: 0 },
    })
    expect(result.rows[0]?.difference).toEqual({ ex_score: 100, min_bp: -5 })
    expect(result.rows[0]?.outcome).toEqual({ ex_score: 'win', min_bp: 'win' })
  })

  test('only compares rows with the complete BMZ score identity', () => {
    const result = buildRivalComparison(
      [score({ chartSha256: 'same', lnPolicy: 'AutoLn' })],
      [
        score({ chartSha256: 'same', lnPolicy: 'ForceLn' }),
        score({ chartSha256: 'same', doubleOption: 'battle' }),
        score({ chartSha256: 'same', ruleMode: 'Dx' }),
        score({ chartSha256: 'same', scoring: 'other_scoring' }),
      ],
    )

    expect(result.rows).toEqual([])
  })

  test('sorts higher levels first and leaves unknown levels last', () => {
    const selfScores = [
      score({ chartSha256: 'unknown', chartTitle: 'Unknown', chartLevel: null }),
      score({ chartSha256: 'low', chartTitle: 'Low', chartLevel: 5 }),
      score({ chartSha256: 'high', chartTitle: 'High', chartLevel: 12 }),
    ]
    const rivalScores = selfScores.map((row) => ({ ...row, scoreId: `rival-${row.scoreId}` }))

    expect(
      buildRivalComparison(selfScores, rivalScores).rows.map((row) => row.chart.sha256),
    ).toEqual(['high', 'low', 'unknown'])
  })
})

function score(overrides: Partial<RivalComparisonScoreRow> = {}): RivalComparisonScoreRow {
  return {
    scoreId: 'score',
    chartSha256: 'chart',
    chartMd5: null,
    chartTitle: 'Chart',
    chartSubtitle: null,
    chartArtist: 'Artist',
    chartMode: 'beat-7k',
    chartLevel: 10,
    chartDifficulty: 'another',
    lnPolicy: 'AutoLn',
    doubleOption: 'off',
    ruleMode: 'Beatoraja',
    scoring: 'bms_ex_score_v1',
    exScore: 1000,
    minBp: 30,
    ...overrides,
  }
}
