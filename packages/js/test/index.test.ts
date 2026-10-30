import { describe, expect, test } from "bun:test";
import fc from "fast-check";
import fixtures from "../../../fixtures/adhan-js.json";
import {
	type Method,
	type Shafaq,
	METHODS,
	PrayerTimesError,
	elevation,
	isRamadan,
	latitude,
	longitude,
	prayerTimes,
} from "../src/index";
import { methods as compiledMethods } from "../src/generated/hya.js";

const PRAYERS = ["fajr", "sunrise", "dhuhr", "asr", "maghrib", "isha"] as const;
const RUNS = Number(process.env["FC_RUNS"] ?? 2000);

const civilDate = fc
	.date({ min: new Date(Date.UTC(1900, 0, 1)), max: new Date(Date.UTC(2100, 11, 31)), noInvalidDate: true })
	.map((d) => ({ year: d.getUTCFullYear(), month: d.getUTCMonth() + 1, day: d.getUTCDate() }));
const method = fc.constantFrom(...METHODS);

describe("branded constructors", () => {
	test("accept exactly the documented ranges", () => {
		fc.assert(
			fc.property(fc.double(), (x) => {
				const ok = Number.isFinite(x) && x >= -90 && x <= 90;
				if (ok) expect<number>(latitude(x)).toBe(x);
				else expect(() => latitude(x)).toThrow(PrayerTimesError);
			}),
			{ numRuns: RUNS },
		);
		expect(() => longitude(180.0001)).toThrow(PrayerTimesError);
		expect(() => elevation(Number.NaN)).toThrow(PrayerTimesError);
	});
});

describe("matches adhan-js fixtures", () => {
	test("every case within one minute", () => {
		for (const c of fixtures.cases) {
			const [year, month, day] = c.date as [number, number, number];
			const t = prayerTimes(
				{ year, month, day },
				{ latitude: latitude(c.latitude), longitude: longitude(c.longitude) },
				// Moonsighting Committee is compared under its own rule; the generic rules differ by design.
				c.method === "moonsighting_committee"
					? { method: "moonsighting_committee", ...(c.shafaq && { shafaq: c.shafaq as Shafaq }) }
					: { method: c.method as Method, highLatitudeRule: "middle_of_the_night" },
			);
			for (const p of PRAYERS) {
				const minutes = t[p].getTime() / 60_000;
				// adhan-js's own Asr drifts up to 2 min above 50° (see crates/engine/tests/asr_reference.rs).
				const tolerance = p === "asr" && Math.abs(c.latitude) > 50 ? 2 : 1;
				expect(Math.abs(minutes - c.unix_minutes[p])).toBeLessThanOrEqual(tolerance);
			}
			// adhan-js's SunnahTimes: the night from Maghrib to the next day's Fajr.
			expect(Math.abs(t.midnight!.getTime() / 60_000 - c.night.midnight)).toBeLessThanOrEqual(1);
			expect(Math.abs(t.lastThird!.getTime() / 60_000 - c.night.last_third)).toBeLessThanOrEqual(1);
		}
	});
});

describe("the night", () => {
	test("runs from Maghrib to the next day's Fajr", () => {
		// Riyadh, 29 October 2026: Maghrib 17:16, the next Fajr 04:40 (UTC+3).
		const riyadh = { latitude: latitude(24.7136), longitude: longitude(46.6753) };
		const t = prayerTimes({ year: 2026, month: 10, day: 29 }, riyadh);
		expect(t.midnight?.toISOString()).toBe("2026-10-29T19:58:00.000Z");
		expect(t.lastThird?.toISOString()).toBe("2026-10-29T21:52:00.000Z");
	});

	test("is null without a next day's Fajr", () => {
		const riyadh = { latitude: latitude(24.7136), longitude: longitude(46.6753) };
		const t = prayerTimes({ year: 2100, month: 12, day: 31 }, riyadh);
		expect(t.isha).toBeInstanceOf(Date);
		expect(t.midnight).toBeNull();
		expect(t.lastThird).toBeNull();
	});
});

describe("properties", () => {
	test("total below 55° and always ordered", () => {
		fc.assert(
			fc.property(civilDate, fc.double({ min: -55, max: 55, noNaN: true }), fc.double({ min: -180, max: 180, noNaN: true }), method, (date, lat, lng, m) => {
				const t = prayerTimes(date, { latitude: latitude(lat), longitude: longitude(lng) }, { method: m });
				const seq = PRAYERS.map((p) => t[p].getTime());
				expect(seq.every(Number.isInteger)).toBe(true);
				for (let i = 1; i < 6; i++) expect(seq[i]!).toBeGreaterThan(seq[i - 1]!);
				expect(t.midnight === null).toBe(t.lastThird === null);
				if (t.midnight !== null && t.lastThird !== null) {
					expect(t.midnight.getTime()).toBeGreaterThan(t.maghrib.getTime());
					expect(t.lastThird.getTime()).toBeGreaterThan(t.midnight.getTime());
				}
			}),
			{ numRuns: RUNS },
		);
	});

	test("never throws anything but PrayerTimesError, anywhere on Earth", () => {
		fc.assert(
			fc.property(civilDate, fc.double({ min: -90, max: 90, noNaN: true }), fc.double({ min: -180, max: 180, noNaN: true }), method, (date, lat, lng, m) => {
				try {
					prayerTimes(date, { latitude: latitude(lat), longitude: longitude(lng) }, { method: m, highLatitudeRule: "none" });
				} catch (error) {
					expect(error).toBeInstanceOf(PrayerTimesError);
					expect((error as PrayerTimesError).code).toBe("compute");
				}
			}),
			{ numRuns: RUNS },
		);
	});
});

describe("examples", () => {
	test("timeZone picks the schedule of that zone's civil day", () => {
		const apia = { latitude: latitude(-13.83), longitude: longitude(-171.76) };
		const day = { year: 2026, month: 10, day: 30 };
		const inApia = new Intl.DateTimeFormat("en-CA", { timeZone: "Pacific/Apia" });
		const solar = prayerTimes(day, apia, { method: "muslim_world_league" });
		expect(inApia.format(solar.dhuhr)).toBe("2026-10-31");
		const civil = prayerTimes(day, apia, { method: "muslim_world_league", timeZone: "Pacific/Apia" });
		for (const prayer of PRAYERS) {
			expect(inApia.format(civil[prayer])).toBe("2026-10-30");
		}
		const riyadh = { latitude: latitude(24.7136), longitude: longitude(46.6753) };
		expect(prayerTimes(day, riyadh, { timeZone: "Asia/Riyadh" })).toEqual(prayerTimes(day, riyadh));
		expect(() => prayerTimes(day, riyadh, { timeZone: "Mars/Olympus" })).toThrow(PrayerTimesError);
	});

	test("tehran keeps Maghrib at sunset when its angle passes Isha", () => {
		const oslo = { latitude: latitude(59.9139), longitude: longitude(10.7522) };
		const june = prayerTimes({ year: 2026, month: 6, day: 1 }, oslo, { method: "tehran" });
		expect(june.notes.maghribAtSunset).toBe(true);
		expect(june.maghrib.getTime()).toBeLessThan(june.isha.getTime());
	});

	test("moonsighting_committee uses its own rule unless told otherwise", () => {
		const london = { latitude: latitude(51.5074), longitude: longitude(-0.1278) };
		const june = { year: 2026, month: 6, day: 21 };
		const own = prayerTimes(june, london, { method: "moonsighting_committee" });
		expect(own).toEqual(prayerTimes(june, london, { method: "moonsighting_committee", highLatitudeRule: "season_adjusted", shafaq: "general" }));
		expect(own.notes.ishaHighLatitude).toBe(true);
		const white = prayerTimes(june, london, { method: "moonsighting_committee", shafaq: "white" });
		expect(white.isha.getTime()).toBeGreaterThan(own.isha.getTime());
		expect(() => prayerTimes(june, london, { method: "umm_al_qura", shafaq: "red" })).toThrow(PrayerTimesError);
	});

	// Exactly the official calculator at ummulqura.org.sa: 04:40 05:58 11:38 14:51 17:16 18:46 local.
	test("defaults are Umm al-Qura: Riyadh 2026-10-29", () => {
		const t = prayerTimes({ year: 2026, month: 10, day: 29 }, { latitude: latitude(24.7136), longitude: longitude(46.6753) });
		const hhmm = (d: Date) => d.toISOString().slice(11, 16);
		expect(PRAYERS.map((p) => hhmm(t[p]))).toEqual(["01:40", "02:58", "08:38", "11:51", "14:16", "15:46"]);
	});

	test("error codes are stable", () => {
		const here = { latitude: latitude(0), longitude: longitude(0) };
		expect(() => prayerTimes({ year: 2026, month: 2, day: 30 }, here)).toThrow(expect.objectContaining({ code: "invalid_input" }));
		expect(() => prayerTimes({ year: 2026, month: 1, day: 1 }, here, { method: "nope" as Method })).toThrow(
			expect.objectContaining({ code: "unknown_method" }),
		);
		expect(() =>
			prayerTimes({ year: 2026, month: 12, day: 21 }, { latitude: latitude(78.2), longitude: longitude(15.6) }),
		).toThrow(expect.objectContaining({ code: "compute" }));
		expect(() => prayerTimes({ year: 2026.5, month: 1, day: 1 }, here)).toThrow(PrayerTimesError);
	});

	test("Ramadan 1447 boundaries", () => {
		expect(isRamadan({ year: 2026, month: 2, day: 17 })).toBe(false);
		expect(isRamadan({ year: 2026, month: 2, day: 18 })).toBe(true);
		expect(isRamadan({ year: 2026, month: 3, day: 19 })).toBe(true);
		expect(isRamadan({ year: 2026, month: 3, day: 20 })).toBe(false);
	});

	test("an announced Ramadan moves the 120-minute Isha", () => {
		const makkah = { latitude: latitude(21.4225), longitude: longitude(39.8262) };
		const ramadan = { first: { year: 2026, month: 2, day: 19 }, last: { year: 2026, month: 3, day: 20 } };
		const gap = (month: number, day: number, announced: boolean) => {
			const t = prayerTimes({ year: 2026, month, day }, makkah, announced ? { ramadan } : {});
			return (t.isha.getTime() - t.maghrib.getTime()) / 60_000;
		};
		expect([gap(2, 18, false), gap(2, 18, true)]).toEqual([120, 90]);
		expect([gap(3, 20, false), gap(3, 20, true)]).toEqual([90, 120]);
		expect(() =>
			prayerTimes({ year: 2026, month: 3, day: 1 }, makkah, {
				ramadan: { first: { year: 2027, month: 2, day: 19 }, last: { year: 2027, month: 3, day: 20 } },
			}),
		).toThrow(expect.objectContaining({ code: "invalid_input" }));
		expect(() =>
			prayerTimes({ year: 2026, month: 3, day: 1 }, makkah, {
				ramadan: { first: { year: 2026, month: 2, day: 19.5 }, last: { year: 2026, month: 3, day: 20 } },
			}),
		).toThrow(PrayerTimesError);
	});

	test("TS method list matches the compiled core", () => {
		expect(compiledMethods()).toEqual([...METHODS]);
	});
});
