// What every place that shows a listing title shares: its length against what a listing takes, the mark of a
// title someone wrote, and the names of a title's parts.

import clsx from "clsx";
import { t, useT } from "../i18n";
import { MAX_TITLE, type TitleField } from "../lib/api";
import { TITLE_FIELDS } from "../lib/titlePattern";

/** Characters as eBay counts them: an emoji or an accented letter is one. */
export function titleLength(text: string): number {
  return [...text].length;
}

/** "76/80", red once the title is longer than a listing takes. */
export function TitleCount({ text, className }: { text: string; className?: string }) {
  const t = useT();
  const count = titleLength(text);
  const over = count > MAX_TITLE;
  return (
    <span
      className={clsx(
        "shrink-0 text-xs tabular-nums",
        over ? "font-semibold text-red-600 dark:text-red-400" : "text-slate-400",
        className,
      )}
      title={over ? t("titles.tooLong", { count, max: MAX_TITLE }) : t("titles.countHint", { max: MAX_TITLE })}
    >
      {t("titles.count", { count, max: MAX_TITLE })}
    </span>
  );
}

/** Marks a title written for the card: templates don't change it. */
export function CustomTitleMark({ className }: { className?: string }) {
  const t = useT();
  return (
    <span
      className={clsx(
        "shrink-0 rounded bg-sky-100 px-1.5 py-0.5 text-[11px] font-medium text-sky-800 dark:bg-sky-950 dark:text-sky-300",
        className,
      )}
      title={t("titles.customHint")}
    >
      {t("titles.custom")}
    </span>
  );
}

export { TITLE_FIELDS };

export function titleFieldLabel(field: TitleField): string {
  return t(`titles.field.${field}`);
}

/** "Left out to fit: Team, Year", or nothing when everything fitted. */
export function leftOutText(fields: TitleField[]): string {
  // A part from a newer version's template has no name here.
  const names = fields.filter((f) => TITLE_FIELDS.includes(f)).map(titleFieldLabel);
  return names.length ? t("titles.leftOut", { fields: names.join(", ") }) : "";
}
