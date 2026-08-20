import { and, desc, eq } from 'drizzle-orm'
import { db, schema } from 'hub:db'
import type { IrRivalComparison, IrRivalsResponse, LnScorePolicy } from '../../shared/types/ir'
import { lookupDifficultyLabels } from './difficulty_tables'
import {
  buildRivalComparison,
  type RivalComparisonScoreRow,
  type RivalMutation,
} from './rivals_core'

export async function rivalTargetExists(playerId: string): Promise<boolean> {
  const rows = await db
    .select({ id: schema.profiles.id })
    .from(schema.profiles)
    .where(eq(schema.profiles.id, playerId))
    .limit(1)
  return rows.length > 0
}

export async function applyRivalMutation(ownerPlayerId: string, mutation: RivalMutation) {
  if (mutation.action === 'remove') {
    await db
      .delete(schema.rivalRelationships)
      .where(
        and(
          eq(schema.rivalRelationships.ownerPlayerId, ownerPlayerId),
          eq(schema.rivalRelationships.targetPlayerId, mutation.targetPlayerId),
          eq(schema.rivalRelationships.relationType, 'rival'),
        ),
      )
    return { removed: true }
  }

  await db
    .insert(schema.rivalRelationships)
    .values({
      ownerPlayerId,
      targetPlayerId: mutation.targetPlayerId,
      relationType: 'rival',
    })
    .onConflictDoNothing()
  return { added: true }
}

export async function removeRival(ownerPlayerId: string, targetPlayerId: string) {
  return applyRivalMutation(ownerPlayerId, { targetPlayerId, action: 'remove' })
}

export async function loadRivalLists(playerId: string): Promise<IrRivalsResponse> {
  const [rivals, reverseRivals] = await Promise.all([
    db
      .select({
        player_id: schema.rivalRelationships.targetPlayerId,
        relation_type: schema.rivalRelationships.relationType,
        created_at: schema.rivalRelationships.createdAt,
        display_name: schema.profiles.displayName,
        bio: schema.profiles.bio,
      })
      .from(schema.rivalRelationships)
      .innerJoin(schema.profiles, eq(schema.profiles.id, schema.rivalRelationships.targetPlayerId))
      .where(
        and(
          eq(schema.rivalRelationships.ownerPlayerId, playerId),
          eq(schema.rivalRelationships.relationType, 'rival'),
        ),
      )
      .orderBy(desc(schema.rivalRelationships.createdAt)),
    db
      .select({
        player_id: schema.rivalRelationships.ownerPlayerId,
        relation_type: schema.rivalRelationships.relationType,
        created_at: schema.rivalRelationships.createdAt,
        display_name: schema.profiles.displayName,
        bio: schema.profiles.bio,
      })
      .from(schema.rivalRelationships)
      .innerJoin(schema.profiles, eq(schema.profiles.id, schema.rivalRelationships.ownerPlayerId))
      .where(
        and(
          eq(schema.rivalRelationships.targetPlayerId, playerId),
          eq(schema.rivalRelationships.relationType, 'rival'),
        ),
      )
      .orderBy(desc(schema.rivalRelationships.createdAt)),
  ])

  return {
    rivals: rivals.map(rivalEntry),
    reverse_rivals: reverseRivals.map(rivalEntry),
  }
}

export async function loadRivalComparison(
  ownerPlayerId: string,
  targetPlayerId: string,
  limit: number,
  offset: number,
): Promise<IrRivalComparison | null> {
  const relation = await db
    .select({ targetPlayerId: schema.rivalRelationships.targetPlayerId })
    .from(schema.rivalRelationships)
    .where(
      and(
        eq(schema.rivalRelationships.ownerPlayerId, ownerPlayerId),
        eq(schema.rivalRelationships.targetPlayerId, targetPlayerId),
        eq(schema.rivalRelationships.relationType, 'rival'),
      ),
    )
    .limit(1)
  if (relation.length === 0) return null

  const [ownerProfile, targetProfile, selfScores, rivalScores] = await Promise.all([
    loadProfile(ownerPlayerId),
    loadProfile(targetPlayerId),
    loadBestScores(ownerPlayerId),
    loadBestScores(targetPlayerId),
  ])
  if (!ownerProfile || !targetProfile) return null

  const comparison = buildRivalComparison(selfScores, rivalScores)
  const pagedRows = comparison.rows.slice(offset, offset + limit)
  const labels = await lookupDifficultyLabels(
    pagedRows.map((row) => ({ sha256: row.chart.sha256, md5: row.chart.md5 })),
  )

  return {
    players: { self: ownerProfile, rival: targetProfile },
    summary: comparison.summary,
    comparisons: pagedRows.map((row) => ({
      ...row,
      difficulty_labels: labels.get(row.chart.sha256) ?? [],
    })),
    pagination: {
      limit,
      offset,
      total: comparison.rows.length,
      has_more: offset + limit < comparison.rows.length,
    },
  }
}

async function loadProfile(playerId: string) {
  const rows = await db
    .select({ id: schema.profiles.id, display_name: schema.profiles.displayName })
    .from(schema.profiles)
    .where(eq(schema.profiles.id, playerId))
    .limit(1)
  return rows[0] ?? null
}

async function loadBestScores(playerId: string): Promise<RivalComparisonScoreRow[]> {
  const rows = await db
    .select({
      scoreId: schema.bestScores.scoreId,
      chartSha256: schema.bestScores.chartSha256,
      chartMd5: schema.charts.md5,
      chartTitle: schema.charts.title,
      chartSubtitle: schema.charts.subtitle,
      chartArtist: schema.charts.artist,
      chartMode: schema.charts.mode,
      chartLevel: schema.charts.level,
      chartDifficulty: schema.charts.difficulty,
      lnPolicy: schema.bestScores.lnPolicy,
      doubleOption: schema.bestScores.doubleOption,
      ruleMode: schema.bestScores.ruleMode,
      scoring: schema.bestScores.scoring,
      exScore: schema.bestScores.exScore,
      minBp: schema.bestScores.minBp,
    })
    .from(schema.bestScores)
    .innerJoin(schema.charts, eq(schema.charts.sha256, schema.bestScores.chartSha256))
    .where(eq(schema.bestScores.playerId, playerId))
  return rows.map((row) => ({ ...row, lnPolicy: row.lnPolicy as LnScorePolicy }))
}

function rivalEntry(row: {
  player_id: string
  relation_type: 'rival'
  created_at: Date
  display_name: string
  bio: string
}) {
  return {
    player_id: row.player_id,
    relation_type: row.relation_type,
    created_at: row.created_at.toISOString(),
    profile: { id: row.player_id, display_name: row.display_name, bio: row.bio || null },
  }
}
