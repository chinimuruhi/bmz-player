import { loadRivalLists } from '../../services/rivals'
import { requireIrUser } from '../../utils/auth'

export default defineEventHandler(async (event) => {
  const user = await requireIrUser(event)
  return loadRivalLists(user.id)
})
