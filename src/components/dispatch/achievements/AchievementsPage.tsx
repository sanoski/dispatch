// Dispatch achievements page (HANDOFF.md, "Feature: Achievements"): the
// voice data card with its fine-tune tiers, then every achievement as a
// crossing-signal card. Hidden ones stay "???" until found. The banner on
// top is ARTWORK.md's "Achievements banner": Joshua's pick, warmer than the
// rest on purpose, shown whole above the title (its left side is too busy
// for text on top).
import React, { useMemo, useState } from "react";
import banner from "@/assets/dispatch/art/achievements-banner.webp";
import { useTranslation } from "react-i18next";
import type { Achievement, AchievementsStatus } from "@/bindings";
import { SignalLamps } from "./SignalLamps";
import {
  TIERS,
  audioLength,
  shortDate,
  useAchievements,
} from "./useAchievements";

type Filter = "all" | "unlocked" | "locked" | "hidden";
const FILTERS: Filter[] = ["all", "unlocked", "locked", "hidden"];
const SECTIONS = ["milestones", "habits", "features", "hidden"] as const;
/** The two secret ones have no hint of their own. */
const NO_HINT: Record<string, string> = {
  pushing_his_buttons: "dispatch.achievements.noHint",
  bedtime_story: "dispatch.achievements.noHintEither",
};

export const AchievementsPage: React.FC = () => {
  const { t } = useTranslation();
  const { status, failed } = useAchievements();
  const [filter, setFilter] = useState<Filter>("all");

  if (!status) {
    return failed ? (
      <p className="w-full max-w-5xl text-sm text-ink-3">
        {t("dispatch.achievements.loadFailed")}
      </p>
    ) : null;
  }

  const cards = status.achievements.filter((a) => a.section !== "voice_data");
  // Keeping only the newest recordings (dispatch_audio.rs), the voice data
  // meter and its training tiers don't apply, so they leave the page and
  // the count.
  const counted = status.keeps_all_audio ? status.achievements : cards;
  const unlocked = counted.filter((a) => a.unlocked_at !== null);
  const hidden = cards.filter((a) => a.section === "hidden");
  const hiddenFound = hidden.filter((a) => a.unlocked_at !== null).length;

  const shows = (a: Achievement) => {
    const done = a.unlocked_at !== null;
    const secret = a.section === "hidden" && !done;
    if (filter === "unlocked") return done;
    if (filter === "locked") return !done && !secret;
    if (filter === "hidden") return a.section === "hidden";
    return true;
  };

  return (
    <div className="flex w-full max-w-5xl flex-col gap-7 px-2 pb-6 pt-2">
      <img
        src={banner}
        alt=""
        width={1536}
        height={512}
        draggable={false}
        className="-mb-2 aspect-[3/1] h-auto w-full rounded-2xl border border-line object-cover shadow-[0_24px_48px_-32px_rgba(0,0,0,0.6)]"
      />
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div className="flex flex-col gap-1.5">
          <h1 className="m-0 font-display text-[28px] font-bold tracking-[-0.4px]">
            {t("dispatch.achievements.title")}
          </h1>
          <p className="m-0 text-[15px] text-ink-2">
            {t("dispatch.achievements.subtitle")}
          </p>
        </div>
        <div className="flex gap-2.5">
          <StatBox
            value={
              <>
                {unlocked.length}
                <span className="text-base text-ink-3">
                  {" "}
                  {t("dispatch.achievements.ofTotal", {
                    total: counted.length,
                  })}
                </span>
              </>
            }
            label={t("dispatch.achievements.unlocked")}
          />
          <StatBox
            value={hidden.length - hiddenFound}
            label={t("dispatch.achievements.hiddenLeft")}
          />
        </div>
      </header>

      {status.keeps_all_audio ? (
        <VoiceDataCard status={status} />
      ) : (
        <p className="m-0 rounded-xl border border-line bg-[var(--color-card)] px-5 py-4 text-sm leading-relaxed text-ink-2">
          {t("dispatch.achievements.voice.notKept")}
        </p>
      )}

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div
          role="group"
          aria-label={t("dispatch.achievements.filters.label")}
          className="flex flex-wrap gap-2"
        >
          {FILTERS.map((id) => {
            const on = id === filter;
            return (
              <button
                key={id}
                type="button"
                aria-pressed={on}
                onClick={() => setFilter(id)}
                className={`min-h-9 cursor-pointer rounded-full border px-3.5 text-[13px] font-semibold transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary ${
                  on
                    ? "border-logo-primary bg-logo-primary/14 text-text"
                    : "border-line-strong text-ink-2 hover:border-ink-3"
                }`}
              >
                {t(`dispatch.achievements.filters.${id}`, {
                  count: cards.length,
                })}
              </button>
            );
          })}
        </div>
        <span className="text-[13px] text-ink-3">
          {t("dispatch.achievements.mystery")}
        </span>
      </div>

      {SECTIONS.map((section) => {
        let list = cards.filter((a) => a.section === section && shows(a));
        if (section === "hidden") {
          // Found ones first, so the mystery cards sit together.
          list = [
            ...list.filter((a) => a.unlocked_at !== null),
            ...list.filter((a) => a.unlocked_at === null),
          ];
        }
        if (list.length === 0) return null;
        return (
          <section key={section} className="flex flex-col gap-3.5">
            <h2 className="m-0 text-[13px] font-semibold uppercase tracking-[0.8px] text-ink-3">
              {section === "hidden"
                ? t("dispatch.achievements.sections.hiddenFound", {
                    found: hiddenFound,
                    total: hidden.length,
                  })
                : t(`dispatch.achievements.sections.${section}`)}
            </h2>
            <div className="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-3.5">
              {list.map((a) => (
                <AchievementCard key={a.id} achievement={a} />
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
};

const StatBox: React.FC<{ value: React.ReactNode; label: string }> = ({
  value,
  label,
}) => (
  <div className="flex flex-col items-end rounded-xl border border-line bg-[var(--color-card)] px-4 py-2.5">
    <span className="font-display text-2xl font-bold tabular-nums">
      {value}
    </span>
    <span className="text-xs text-ink-3">{label}</span>
  </div>
);

const VoiceDataCard: React.FC<{ status: AchievementsStatus }> = ({
  status,
}) => {
  const { t, i18n } = useTranslation();
  const recorded = status.recorded_ms;
  const checked = status.checked_ms;
  const total = audioLength(recorded);
  const tierState = useMemo(
    () =>
      TIERS.map((tier, i) => {
        const from = i === 0 ? 0 : TIERS[i - 1].minutes * 60_000;
        const to = tier.minutes * 60_000;
        const share = (ms: number) =>
          Math.max(0, Math.min(1, (ms - from) / (to - from)));
        const unlock = status.achievements.find((a) => a.id === tier.id);
        return {
          ...tier,
          to,
          fill: share(recorded),
          checkedFill: share(checked),
          reachedAt: unlock?.unlocked_at ?? null,
        };
      }),
    [status, recorded, checked],
  );
  const current = tierState.find(
    (tier) => tier.reachedAt === null && tier.fill > 0,
  );
  const upcoming = tierState
    .filter((tier) => tier.reachedAt === null)
    .slice(0, 2);
  const perDay = status.per_day_ms;

  return (
    <section
      aria-labelledby="dispatch-voice-data"
      className="flex flex-col gap-5 rounded-2xl border border-line bg-[var(--color-card)] px-6 py-6"
    >
      <div className="flex flex-wrap items-start justify-between gap-5">
        <div className="flex flex-col gap-1.5">
          <span className="text-xs font-semibold uppercase tracking-[0.8px] text-ink-3">
            {t("dispatch.achievements.voice.eyebrow")}
          </span>
          <h2
            id="dispatch-voice-data"
            className="m-0 flex items-baseline gap-2.5"
          >
            <span className="font-display text-[42px] font-bold leading-none tracking-[-0.8px] tabular-nums">
              {total.value} {total.unit}
            </span>
            <span className="text-[15px] font-medium text-ink-2">
              {t("dispatch.achievements.voice.soFar")}
            </span>
          </h2>
          <p className="m-0 max-w-xl text-sm leading-normal text-ink-2">
            {t("dispatch.achievements.voice.explain")}
          </p>
        </div>
        <div className="flex flex-col items-end gap-1 rounded-xl border border-line bg-background px-4 py-3 text-right">
          {upcoming.length === 0 ? (
            <span className="text-[15px] font-semibold">
              {t("dispatch.achievements.voice.allTiers")}
            </span>
          ) : perDay && perDay > 0 ? (
            <>
              <span className="text-xs text-ink-3">
                {t("dispatch.achievements.voice.pace", {
                  amount: paceAmount(perDay),
                })}
              </span>
              {upcoming.map((tier, i) => {
                const days = Math.ceil((tier.to - recorded) / perDay);
                return (
                  <span
                    key={tier.id}
                    className={
                      i === 0
                        ? "text-[15px] font-semibold"
                        : "text-[13px] text-ink-2"
                    }
                  >
                    {t("dispatch.achievements.voice.eta", {
                      tier: t(`dispatch.achievements.tiers.${tier.id}.name`),
                      count: Math.max(1, days),
                    })}
                  </span>
                );
              })}
            </>
          ) : (
            <span className="text-[13px] text-ink-2">
              {t("dispatch.achievements.voice.noPace")}
            </span>
          )}
        </div>
      </div>

      <div className="flex flex-col gap-2.5">
        <div
          role="img"
          aria-label={t("dispatch.achievements.voice.barLabel", {
            recorded: `${total.value} ${total.unit}`,
            tier: current
              ? t(`dispatch.achievements.tiers.${current.id}.name`)
              : "",
          })}
          className="grid grid-cols-4 gap-1.5"
        >
          {tierState.map((tier) => (
            <div key={tier.id} className="flex flex-col gap-1">
              <div className="relative h-3.5 overflow-hidden rounded-full border border-line bg-[var(--color-chrome)]">
                <div
                  className="absolute inset-y-0 left-0 rounded-full bg-logo-primary transition-[width] duration-700"
                  style={{ width: `${(tier.fill * 100).toFixed(1)}%` }}
                />
              </div>
              <div className="h-1 overflow-hidden rounded-full bg-[var(--color-chrome)]">
                <div
                  className="h-full rounded-full bg-warning"
                  style={{ width: `${(tier.checkedFill * 100).toFixed(1)}%` }}
                />
              </div>
            </div>
          ))}
        </div>
        <div className="grid grid-cols-2 gap-x-1.5 gap-y-3 md:grid-cols-4">
          {tierState.map((tier) => {
            const reached = tier.reachedAt !== null;
            const isCurrent = tier === current;
            return (
              <div key={tier.id} className="flex items-start gap-2.5 pt-1">
                <SignalLamps
                  size="tier"
                  state={reached ? "lit" : isCurrent ? "half" : "dark"}
                />
                <div className="flex min-w-0 flex-col gap-0.5">
                  <span className="text-sm font-semibold">
                    {t(`dispatch.achievements.tiers.${tier.id}.name`)}{" "}
                    <span className="font-medium text-ink-3">
                      {t("dispatch.achievements.voice.tierAmount", {
                        amount: t(
                          `dispatch.achievements.tiers.${tier.id}.amount`,
                        ),
                      })}
                    </span>
                  </span>
                  <span className="text-[13px] leading-snug text-ink-2">
                    {t(`dispatch.achievements.tiers.${tier.id}.meaning`)}
                  </span>
                  <span
                    className={`text-xs font-semibold ${
                      reached || isCurrent ? "text-logo-primary" : "text-ink-3"
                    }`}
                  >
                    {reached
                      ? t("dispatch.achievements.voice.reached", {
                          date: shortDate(tier.reachedAt!, i18n.language),
                        })
                      : isCurrent
                        ? t("dispatch.achievements.voice.toGo", {
                            amount: lengthText(tier.to - recorded),
                          })
                        : t("dispatch.achievements.voice.locked")}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      <div className="flex flex-wrap gap-x-5 gap-y-2 border-t border-line pt-4 text-[13px] text-ink-2">
        <span className="flex items-center gap-2">
          <span className="h-2 w-4.5 rounded-full bg-logo-primary" />
          {t("dispatch.achievements.voice.recorded")}
        </span>
        <span className="flex items-center gap-2">
          <span className="h-1 w-4.5 rounded-full bg-warning" />
          {checked > 0
            ? t("dispatch.achievements.voice.checked", {
                amount: lengthText(checked),
              })
            : t("dispatch.achievements.voice.checkedNone")}
        </span>
        <span className="ms-auto text-ink-3">
          {t("dispatch.achievements.voice.stats", {
            dictations: t("dispatch.achievements.voice.dictations", {
              count: status.dictations,
            }),
            words: t("dispatch.achievements.voice.words", {
              count: status.words,
            }),
            days: t("dispatch.achievements.voice.days", {
              count: status.days_used,
            }),
          })}
        </span>
      </div>
    </section>
  );
};

/** "8.8 min" or "1 h 05 min". */
const lengthText = (ms: number) => {
  const { value, unit } = audioLength(Math.max(0, ms));
  return `${value} ${unit}`;
};

/** Per-day pace, rounded to whole minutes: "21 min", "1 h 05 min". */
const paceAmount = (ms: number) => {
  const minutes = Math.max(1, Math.round(ms / 60_000));
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")} min`;
};

const AchievementCard: React.FC<{ achievement: Achievement }> = ({
  achievement: a,
}) => {
  const { t, i18n } = useTranslation();
  const done = a.unlocked_at !== null;
  const secret = a.section === "hidden" && !done;
  const key = `dispatch.achievements.items.${a.id}`;
  const progress =
    !done && a.have !== null && a.need !== null && a.need > 0
      ? Math.min(1, a.have / a.need)
      : null;
  const number = (n: number) => n.toLocaleString(i18n.language);

  return (
    <div
      className={`flex min-h-[172px] flex-col gap-3 rounded-[14px] p-4 ${
        done
          ? "border border-logo-primary/45 bg-[var(--color-card)] shadow-[0_0_0_1px_color-mix(in_srgb,var(--color-logo-primary),transparent_92%),0_14px_30px_-22px_color-mix(in_srgb,var(--color-logo-primary),transparent_40%)]"
          : secret
            ? "border-[1.5px] border-dashed border-line-strong bg-background"
            : "border border-line bg-[color-mix(in_srgb,var(--color-card)_70%,var(--color-background))]"
      }`}
    >
      <div className="flex items-center justify-between">
        <SignalLamps
          state={done ? "lit" : secret ? "secret" : "dark"}
          label={t(
            done
              ? "dispatch.achievements.lamp.lit"
              : secret
                ? "dispatch.achievements.lamp.hidden"
                : "dispatch.achievements.lamp.locked",
          )}
        />
        <span
          className={`rounded-full px-2.5 py-0.5 text-xs font-semibold ${
            done
              ? "bg-logo-primary text-[var(--color-on-accent)]"
              : "border border-line-strong text-ink-3"
          }`}
        >
          {done
            ? t(
                a.section === "hidden"
                  ? "dispatch.achievements.tag.found"
                  : "dispatch.achievements.tag.unlocked",
                { date: shortDate(a.unlocked_at!, i18n.language) },
              )
            : t(
                secret
                  ? "dispatch.achievements.tag.hidden"
                  : "dispatch.achievements.tag.locked",
              )}
        </span>
      </div>
      <div className="flex flex-col gap-1">
        <span
          className={`font-display text-[17px] font-bold ${
            done ? "text-text" : secret ? "text-ink-3" : "text-ink-2"
          }`}
        >
          {secret ? t("dispatch.achievements.secretTitle") : t(`${key}.title`)}
        </span>
        <span
          className={`text-[13px] leading-[1.45] ${done ? "text-ink-2" : "text-ink-3"}`}
        >
          {secret ? t(NO_HINT[a.id] ?? `${key}.hint`) : t(`${key}.desc`)}
        </span>
      </div>
      {progress !== null && (
        <div className="mt-auto flex flex-col gap-1.5">
          <div className="h-1.5 overflow-hidden rounded-full bg-[var(--color-chrome)]">
            <div
              className="h-full rounded-full bg-logo-primary"
              style={{ width: `${(progress * 100).toFixed(1)}%` }}
            />
          </div>
          <span className="text-xs tabular-nums text-ink-3">
            {t(`${key}.progress`, {
              have: number(a.have!),
              need: number(a.need!),
              defaultValue: `${number(a.have!)} / ${number(a.need!)}`,
            })}
          </span>
        </div>
      )}
    </div>
  );
};
