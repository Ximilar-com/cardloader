import type { Settings } from "./api";

type Fields = Record<string, unknown>;

const isFields = (v: unknown): v is Fields => !!v && typeof v === "object" && !Array.isArray(v);
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

function rebase(base: Fields, draft: Fields, current: Fields, depth: number): Fields {
  const out = { ...current };
  for (const [key, mine] of Object.entries(draft)) {
    const [old, theirs] = [base[key], current[key]];
    if (same(mine, old)) continue;
    out[key] = depth > 0 && isFields(mine) && isFields(old) && isFields(theirs) ? rebase(old, mine, theirs, 0) : mine;
  }
  return out;
}

/**
 * A settings draft moved onto what is stored now: what the user changed in the draft (against `base`, the stored
 * settings it started from) is kept, everything else comes from `current`. So saving the draft doesn't undo what
 * the New session dialog, the setup wizard or an agent saved meanwhile. Nested settings (`scan`, `ebay`, `mcp`)
 * merge field by field, so changing one scan option doesn't pin the others.
 */
export function rebaseSettings(base: Settings, draft: Settings, current: Settings): Settings {
  return rebase({ ...base }, { ...draft }, { ...current }, 1) as unknown as Settings;
}
