import { removeRival } from '../../../services/rivals'
import { requireIrUser } from '../../../utils/auth'

export default defineEventHandler(async (event) => {
  const user = await requireIrUser(event)
  const targetPlayerId = getRouterParam(event, 'playerId')?.trim()
  if (!targetPlayerId || targetPlayerId === user.id) {
    throw createError({ statusCode: 400, statusMessage: 'valid player id is required' })
  }
  return removeRival(user.id, targetPlayerId)
})
