import { getQuery } from 'h3'
import { clampInteger } from '../../../../services/ir/common'
import { loadRivalComparison } from '../../../../services/rivals'
import { requireIrUser } from '../../../../utils/auth'

export default defineEventHandler(async (event) => {
  const user = await requireIrUser(event)
  const targetPlayerId = getRouterParam(event, 'playerId')?.trim()
  if (!targetPlayerId || targetPlayerId === user.id) {
    throw createError({ statusCode: 400, statusMessage: 'valid player id is required' })
  }

  const query = getQuery(event)
  const limit = clampInteger(query.limit, 50, 1, 100)
  const offset = clampInteger(query.offset, 0, 0, 100_000)
  const comparison = await loadRivalComparison(user.id, targetPlayerId, limit, offset)
  if (!comparison) {
    throw createError({ statusCode: 404, statusMessage: 'Rival not found' })
  }
  return comparison
})
