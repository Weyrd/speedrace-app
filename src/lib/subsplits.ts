export interface ParsedSegment {
  label: string;
  isSubsplit: boolean;
  sectionName?: string;
}

export function parseSegmentName(raw: string): ParsedSegment {
  const trimmed = raw.trim();
  const sectionMatch = trimmed.match(/^\{([^}]*)\}\s*(.*)$/);
  if (sectionMatch) {
    const sectionName = sectionMatch[1].trim();
    let rest = sectionMatch[2].trim();
    const isSubsplit = rest.startsWith("-");
    if (isSubsplit) rest = rest.slice(1).trim();
    return { label: rest === "" ? sectionName : rest, isSubsplit, sectionName };
  }
  if (trimmed.startsWith("-")) {
    return { label: trimmed.slice(1).trim(), isSubsplit: true };
  }
  return { label: trimmed, isSubsplit: false };
}

export interface SegmentSplit {
  index: number;
  label: string;
  isSubsplit: boolean;
}

export interface SegmentSection {
  name: string | null;
  splits: SegmentSplit[];
}

// A "{Section}" segment is the section's last/major split, so it CLOSES the group.
export function groupSegments(
  rawNames: ReadonlyArray<string>,
): SegmentSection[] {
  const sections: SegmentSection[] = [];
  let pending: SegmentSplit[] = [];
  rawNames.forEach((raw, index) => {
    const parsed = parseSegmentName(raw);
    pending.push({ index, label: parsed.label, isSubsplit: parsed.isSubsplit });
    if (parsed.sectionName != null) {
      sections.push({ name: parsed.sectionName, splits: pending });
      pending = [];
    }
  });
  if (pending.length > 0) sections.push({ name: null, splits: pending });
  return sections;
}

export function hasSubsplitStructure(rawNames: ReadonlyArray<string>): boolean {
  return rawNames.some((n) => {
    const t = n.trim();
    return t.startsWith("-") || /^\{[^}]*\}/.test(t);
  });
}
