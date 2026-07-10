import type {
  CatalogEntryResponse,
  ReplacePolicyItem,
} from '@/hooks/usePermissionsAdmin'

export const POLICY_ACTIONS: ReplacePolicyItem['action'][] = [
  'read',
  'write',
  'approve',
  '*',
]

export function policyActionsForObject(
  catalog: CatalogEntryResponse[] | undefined,
  object: string,
): ReplacePolicyItem['action'][] {
  if (!object || object === '*') return POLICY_ACTIONS
  const entry = catalog?.find((item) => item.object === object)
  if (!entry) return POLICY_ACTIONS
  return [...entry.actions, '*'] as ReplacePolicyItem['action'][]
}

export function normalizePolicyAction(
  catalog: CatalogEntryResponse[] | undefined,
  object: string,
  action: ReplacePolicyItem['action'],
): ReplacePolicyItem['action'] {
  const actions = policyActionsForObject(catalog, object)
  return actions.includes(action) ? action : actions[0]!
}
