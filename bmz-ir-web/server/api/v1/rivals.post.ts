import { readBody } from 'h3'
import { applyRivalMutation, rivalTargetExists } from '../../services/rivals'
import { parseRivalMutation } from '../../services/rivals_core'
import { requireIrUser } from '../../utils/auth'

export default defineEventHandler(async (event) => {
  const user = await requireIrUser(event)
  let mutation
  try {
    mutation = parseRivalMutation(await readBody(event), user.id)
  } catch (error) {
    throw createError({
      statusCode: 400,
      statusMessage: error instanceof Error ? error.message : 'invalid rival payload',
    })
  }

  if (!(await rivalTargetExists(mutation.targetPlayerId))) {
    throw createError({ statusCode: 404, statusMessage: 'Player not found' })
  }
  return applyRivalMutation(user.id, mutation)
})
