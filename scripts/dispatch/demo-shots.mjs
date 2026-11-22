/**
 * Dispatch: screenshots of the real interface running on demo data
 * (src/dispatch-demo/), for the README and the video. Needs the dev server:
 *
 *   bunx vite dev
 *   node scripts/dispatch/demo-shots.mjs [out-dir] [shot ...]
 *
 * Uses the Edge installed with Windows, through Playwright. Prints any
 * command the demo stand-in doesn't answer yet, and the page's errors.
 */
import { chromium } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..", "..");
const OUT = resolve(process.argv[2] ?? join(ROOT, "docs", "dispatch"));
const ONLY = process.argv.slice(3);
const URL = "http://localhost:1420/src/dispatch-demo/";

/** Each shot: a name, the window size, and the clicks that get there. */
const openPage = (label) => async (p) => {
  await p.getByText(label, { exact: true }).first().click();
  await p.waitForTimeout(1200);
};

const SHOTS = [
  {
    name: "general",
    width: 1280,
    height: 820,
    steps: async () => {},
  },
  {
    name: "history",
    width: 1280,
    height: 820,
    steps: async (p) => {
      await openPage("History")(p);
      // The dictation with a fix: "quad code" heard, "Claude Code" meant.
      await p
        .getByText("Can you open up Claude Code", { exact: false })
        .click();
      await p.waitForTimeout(900);
      await p.getByText("What Dispatch heard", { exact: true }).click();
      await p.waitForTimeout(500);
      // Pick the fixed words, then let their playback finish.
      await p.getByRole("button", { name: "Claude", exact: true }).click();
      await p
        .getByRole("button", { name: "Code", exact: true })
        .click({ modifiers: ["Shift"] });
      await p.waitForTimeout(2500);
    },
  },
  {
    name: "read-aloud",
    width: 1280,
    height: 820,
    steps: async (p) => {
      await openPage("Read Aloud")(p);
      await p
        .locator("#dispatch-read-aloud-text")
        .fill(
          "The 4:15 to Riverside is running about ten minutes late this afternoon. " +
            "Passengers for the northbound connection should stay on board at Maple Junction, " +
            "where the conductor will hold the train until everyone has made the transfer.\n\n" +
            "Thank you for your patience, and mind the gap as you leave the train.",
        );
      await p.locator("#dispatch-read-aloud-text").blur();
    },
  },
  {
    name: "achievements",
    width: 1280,
    height: 1135,
    steps: openPage("Achievements"),
  },
  {
    // GitHub's social preview and the README banner: exactly 1280 x 640.
    name: "social-preview",
    url: "http://localhost:1420/src/dispatch-demo/banner.html",
    width: 1280,
    height: 640,
    scale: 1,
    steps: async () => {},
  },
];

const browser = await chromium.launch({ channel: "msedge" });
mkdirSync(OUT, { recursive: true });
for (const shot of SHOTS) {
  if (ONLY.length && !ONLY.includes(shot.name)) continue;
  const page = await browser.newPage({
    viewport: { width: shot.width, height: shot.height },
    deviceScaleFactor: shot.scale ?? 2,
    colorScheme: "dark",
  });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  await page.goto(shot.url ?? URL);
  await page.waitForTimeout(2500);
  await shot.steps(page);
  await page.waitForTimeout(800);
  await page.screenshot({ path: join(OUT, `${shot.name}.png`) });
  const missing = await page.evaluate(() => window.__dispatchDemoMissing ?? []);
  console.log(`${shot.name}: saved`);
  if (missing.length) console.log(`  not answered: ${missing.join(", ")}`);
  for (const e of [...new Set(errors)].slice(0, 10))
    console.log(`  error: ${e.slice(0, 200)}`);
  await page.close();
}
await browser.close();
