// Generates fixtures/asr-reference.json: Asr from first principles, as an independent check that
// does not trust adhan-js (which drifts up to ~2 min at high latitudes; see tests/asr_reference.rs).
// A different solar model from the core's (NOAA / Meeus ch. 25, not USNO), the literal definition
// (noon shadow from the declination at transit), and bisection on the sun's true altitude.
// Run: bun scripts/generate-asr-reference.ts
import { writeFileSync } from "node:fs";
import { join } from "node:path";

const rad = (d: number) => (d * Math.PI) / 180;
const deg = (r: number) => (r * 180) / Math.PI;

/** Declination (degrees) and equation of time (minutes) at a Julian Day. */
function sun(jd: number): [number, number] {
  const t = (jd - 2451545) / 36525;
  const l0 = (280.46646 + t * (36000.76983 + t * 0.0003032)) % 360;
  const m = 357.52911 + t * (35999.05029 - 0.0001537 * t);
  const e = 0.016708634 - t * (0.000042037 + 0.0000001267 * t);
  const c =
    Math.sin(rad(m)) * (1.914602 - t * (0.004817 + 0.000014 * t)) +
    Math.sin(rad(2 * m)) * (0.019993 - 0.000101 * t) +
    Math.sin(rad(3 * m)) * 0.000289;
  const omega = 125.04 - 1934.136 * t;
  const lambda = l0 + c - 0.00569 - 0.00478 * Math.sin(rad(omega));
  const epsilon =
    23 + (26 + (21.448 - t * (46.815 + t * (0.00059 - t * 0.001813))) / 60) / 60 + 0.00256 * Math.cos(rad(omega));
  const declination = deg(Math.asin(Math.sin(rad(epsilon)) * Math.sin(rad(lambda))));
  const y = Math.tan(rad(epsilon / 2)) ** 2;
  const eot =
    4 *
    deg(
      y * Math.sin(2 * rad(l0)) -
        2 * e * Math.sin(rad(m)) +
        4 * e * y * Math.sin(rad(m)) * Math.cos(2 * rad(l0)) -
        0.5 * y * y * Math.sin(4 * rad(l0)) -
        1.25 * e * e * Math.sin(2 * rad(m)),
    );
  return [declination, eot];
}

/** Asr in fractional minutes since the Unix epoch. */
function asr(days: number, latitude: number, longitude: number, factor: number): number {
  const jd0 = 2440587.5 + days;
  let transit = 720 - 4 * longitude;
  for (let i = 0; i < 10; i++) transit = 720 - 4 * longitude - sun(jd0 + transit / 1440)[1];
  const noonDeclination = sun(jd0 + transit / 1440)[0];
  const target = deg(Math.atan(1 / (factor + Math.tan(rad(Math.abs(latitude - noonDeclination))))));
  const altitude = (minute: number) => {
    const [declination, eot] = sun(jd0 + minute / 1440);
    const hourAngle = rad((minute + eot + 4 * longitude) / 4 - 180);
    return deg(
      Math.asin(
        Math.sin(rad(latitude)) * Math.sin(rad(declination)) +
          Math.cos(rad(latitude)) * Math.cos(rad(declination)) * Math.cos(hourAngle),
      ),
    );
  };
  let [lo, hi] = [transit, transit + 600];
  for (let i = 0; i < 80; i++) {
    const mid = (lo + hi) / 2;
    if (altitude(mid) > target) lo = mid;
    else hi = mid;
  }
  return days * 1440 + lo;
}

// Equator to 60°, both hemispheres, both Asr conventions, every season.
const CITIES: [string, number, number][] = [
  ["singapore", 1.3521, 103.8198],
  ["riyadh", 24.7136, 46.6753],
  ["cape_town", -33.9249, 18.4241],
  ["chicago", 41.8781, -87.6298],
  ["london", 51.5074, -0.1278],
  ["glasgow", 55.8642, -4.2518],
  ["oslo", 59.9139, 10.7522],
  ["punta_arenas", -53.1638, -70.9171],
];

const cases = [];
for (const [city, latitude, longitude] of CITIES) {
  for (const factor of [1, 2]) {
    for (let month = 1; month <= 12; month++) {
      for (const day of [1, 15]) {
        const days = Date.UTC(2026, month - 1, day) / 86_400_000;
        const unix_minutes = Math.round(asr(days, latitude, longitude, factor) * 1000) / 1000;
        cases.push({ city, latitude, longitude, factor, date: [2026, month, day], unix_minutes });
      }
    }
  }
}

const out = join(import.meta.dir, "..", "fixtures", "asr-reference.json");
writeFileSync(out, JSON.stringify({ source: "NOAA solar model, exact noon-shadow definition", cases }, null, 1) + "\n");
console.log(`wrote ${cases.length} cases to ${out}`);
