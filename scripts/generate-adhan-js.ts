// Generates fixtures/adhan-js.json: golden prayer times from adhan-js, an independent implementation
// (Jean Meeus' higher-precision algorithms) that the Rust core must agree with to ±1 minute.
// Run: bun scripts/generate-adhan-js.ts
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import * as adhan from "adhan";

type MethodId =
  | "umm_al_qura" | "muslim_world_league" | "egyptian" | "karachi" | "north_america"
  | "dubai" | "kuwait" | "qatar" | "singapore" | "turkey" | "tehran" | "moonsighting_committee";
type Shafaq = "general" | "red" | "white";

const METHODS: Record<MethodId, () => adhan.CalculationParameters> = {
  umm_al_qura: adhan.CalculationMethod.UmmAlQura,
  muslim_world_league: adhan.CalculationMethod.MuslimWorldLeague,
  egyptian: adhan.CalculationMethod.Egyptian,
  karachi: adhan.CalculationMethod.Karachi,
  north_america: adhan.CalculationMethod.NorthAmerica,
  dubai: adhan.CalculationMethod.Dubai,
  kuwait: adhan.CalculationMethod.Kuwait,
  qatar: adhan.CalculationMethod.Qatar,
  singapore: adhan.CalculationMethod.Singapore,
  turkey: adhan.CalculationMethod.Turkey,
  tehran: adhan.CalculationMethod.Tehran,
  moonsighting_committee: adhan.CalculationMethod.MoonsightingCommittee,
};

const SHAFAQ: Record<Shafaq, adhan.CalculationParameters["shafaq"]> = {
  general: adhan.Shafaq.General,
  red: adhan.Shafaq.Ahmer,
  white: adhan.Shafaq.Abyad,
};

// Each city with the method its authorities use. Outside Moonsighting Committee, latitudes stay
// below 48° so no high-latitude rule is involved; the two implementations' generic rules differ
// by design. Moonsighting Committee's own rule is compared everywhere, including above 55°.
const CITIES: [string, number, number, MethodId, Shafaq?][] = [
  ["riyadh", 24.7136, 46.6753, "umm_al_qura"],
  ["makkah", 21.4225, 39.8262, "umm_al_qura"],
  ["jeddah", 21.5433, 39.1728, "umm_al_qura"],
  ["cairo", 30.0444, 31.2357, "egyptian"],
  ["karachi", 24.8607, 67.0011, "karachi"],
  ["toronto", 43.6532, -79.3832, "north_america"],
  ["dubai", 25.2048, 55.2708, "dubai"],
  ["kuwait_city", 29.3759, 47.9774, "kuwait"],
  ["doha", 25.2854, 51.531, "qatar"],
  ["singapore", 1.3521, 103.8198, "singapore"],
  ["istanbul", 41.0082, 28.9784, "turkey"],
  ["tehran", 35.6892, 51.389, "tehran"],
  ["jakarta", -6.2088, 106.8456, "singapore"],
  ["cape_town", -33.9249, 18.4241, "muslim_world_league"],
  ["los_angeles", 34.0522, -118.2437, "north_america"],
  ["london", 51.5074, -0.1278, "moonsighting_committee"],
  ["london_red", 51.5074, -0.1278, "moonsighting_committee", "red"],
  ["london_white", 51.5074, -0.1278, "moonsighting_committee", "white"],
  ["birmingham", 52.4862, -1.8904, "moonsighting_committee"],
  ["glasgow", 55.8642, -4.2518, "moonsighting_committee"],
  ["chicago", 41.8781, -87.6298, "moonsighting_committee"],
  ["sydney", -33.8688, 151.2093, "moonsighting_committee"],
];

const ramadan = new Intl.DateTimeFormat("en-u-ca-islamic-umalqura", { month: "numeric", timeZone: "UTC" });
const isRamadan = (y: number, m: number, d: number) => ramadan.format(Date.UTC(y, m - 1, d)) === "9";

const cases = [];
for (const [city, latitude, longitude, method, shafaq] of CITIES) {
  for (let month = 1; month <= 12; month++) {
    for (const day of [1, 15]) {
      const params = METHODS[method]();
      params.highLatitudeRule = adhan.HighLatitudeRule.MiddleOfTheNight;
      if (shafaq) params.shafaq = SHAFAQ[shafaq];
      const t = new adhan.PrayerTimes(new adhan.Coordinates(latitude, longitude), new Date(2026, month - 1, day), params);
      const minutes = (d: Date) => Math.round(d.getTime() / 60_000);
      const times = {
        fajr: minutes(t.fajr), sunrise: minutes(t.sunrise), dhuhr: minutes(t.dhuhr),
        asr: minutes(t.asr), maghrib: minutes(t.maghrib), isha: minutes(t.isha),
      };
      // adhan-js leaves Umm al-Qura's Ramadan Isha (120 min instead of 90) to the caller.
      // Applied by calendar date, as the Umm al-Qura timetable does.
      if (method === "umm_al_qura" && isRamadan(2026, month, day)) {
        times.isha += 30;
      }
      cases.push({ city, latitude, longitude, method, ...(shafaq && { shafaq }), date: [2026, month, day], unix_minutes: times });
    }
  }
}

const out = join(import.meta.dir, "..", "fixtures", "adhan-js.json");
writeFileSync(out, JSON.stringify({ source: "adhan@4.4.4", year: 2026, cases }, null, 1) + "\n");
console.log(`wrote ${cases.length} cases to ${out}`);
