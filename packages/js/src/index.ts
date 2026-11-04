import { compute as wasmCompute, initSync, isRamadan as wasmIsRamadan } from "./generated/hya.js";
import { WASM_BASE64 } from "./generated/wasm-bytes.js";

initSync({ module: Uint8Array.from(atob(WASM_BASE64), (c) => c.charCodeAt(0)) });

declare const brand: unique symbol;
export type Brand<T, B extends string> = T & { readonly [brand]: B };

export type Latitude = Brand<number, "Latitude">;
export type Longitude = Brand<number, "Longitude">;
export type Elevation = Brand<number, "Elevation">;

export class PrayerTimesError extends Error {
	override readonly name = "PrayerTimesError";
	constructor(
		readonly code: "invalid_input" | "unknown_method" | "compute",
		message: string,
	) {
		super(message);
	}
}

function bounded<B extends string>(name: B, min: number, max: number) {
	return (value: number): Brand<number, B> => {
		if (!Number.isFinite(value) || value < min || value > max) {
			throw new PrayerTimesError("invalid_input", `${name} must be a finite number within ${min}..=${max}, got ${value}`);
		}
		return value as Brand<number, B>;
	};
}

export const latitude = bounded("Latitude", -90, 90);
export const longitude = bounded("Longitude", -180, 180);
export const elevation = bounded("Elevation", -500, 9000);

export const METHODS = [
	"umm_al_qura",
	"muslim_world_league",
	"egyptian",
	"karachi",
	"north_america",
	"dubai",
	"kuwait",
	"qatar",
	"singapore",
	"turkey",
	"tehran",
	"moonsighting_committee",
	"malaysia",
	"malaysia_kelantan",
	"jordan",
] as const;
export type Method = (typeof METHODS)[number];
export type Asr = "standard" | "hanafi";
export type HighLatitudeRule =
	| "recommended"
	| "middle_of_the_night"
	| "seventh_of_the_night"
	| "twilight_angle"
	| "none"
	| "season_adjusted";
export type Shafaq = "general" | "red" | "white";

export interface CivilDate {
	readonly year: number;
	readonly month: number;
	readonly day: number;
}

export interface Location {
	readonly latitude: Latitude;
	readonly longitude: Longitude;
	readonly elevation?: Elevation;
}

export interface Options {
	readonly method?: Method;
	readonly asr?: Asr;
	readonly highLatitudeRule?: HighLatitudeRule;
	readonly shafaq?: Shafaq;
	readonly ramadan?: { readonly first: CivilDate; readonly last: CivilDate };
	readonly timeZone?: string;
}

export interface PrayerTimes {
	readonly fajr: Date;
	readonly sunrise: Date;
	readonly dhuhr: Date;
	readonly asr: Date;
	readonly maghrib: Date;
	readonly isha: Date;
	readonly midnight: Date | null;
	readonly lastThird: Date | null;
	readonly notes: {
		readonly fajrHighLatitude: boolean;
		readonly ishaHighLatitude: boolean;
		readonly ramadanIsha: boolean;
		readonly maghribAtSunset: boolean;
	};
}

function rethrow(error: unknown): never {
	const text = typeof error === "string" ? error : String(error);
	const [code, ...rest] = text.split(": ");
	if (code === "invalid_input" || code === "unknown_method" || code === "compute") {
		throw new PrayerTimesError(code, rest.join(": "));
	}
	throw error;
}

function u8(value: number, what: string): number {
	if (!Number.isInteger(value) || value < 0 || value > 255) {
		throw new PrayerTimesError("invalid_input", `${what} must be an integer, got ${value}`);
	}
	return value;
}

function i16(value: number): number {
	if (!Number.isInteger(value) || value < -32768 || value > 32767) {
		throw new PrayerTimesError("invalid_input", `year must be an integer, got ${value}`);
	}
	return value;
}

function ramadanDates(ramadan: Options["ramadan"]): Int16Array {
	if (ramadan === undefined) return new Int16Array(0);
	const { first, last } = ramadan;
	return Int16Array.from(
		[first.year, first.month, first.day, last.year, last.month, last.day].map((value) => {
			if (!Number.isInteger(value) || value < -32768 || value > 32767) {
				throw new PrayerTimesError("invalid_input", `ramadan dates must be integers, got ${value}`);
			}
			return value;
		}),
	);
}

const dateFormats = new Map<string, Intl.DateTimeFormat>();

function dateFormat(timeZone: string): Intl.DateTimeFormat {
	let format = dateFormats.get(timeZone);
	if (format === undefined) {
		try {
			format = new Intl.DateTimeFormat("en-CA", { timeZone, year: "numeric", month: "2-digit", day: "2-digit" });
		} catch {
			throw new PrayerTimesError("invalid_input", `unknown time zone ${timeZone}`);
		}
		dateFormats.set(timeZone, format);
	}
	return format;
}

export function prayerTimes(date: CivilDate, location: Location, options: Options = {}): PrayerTimes {
	const times = solarDay(date, location, options);
	if (options.timeZone === undefined) {
		return times;
	}
	const format = dateFormat(options.timeZone);
	const wanted = `${String(date.year).padStart(4, "0")}-${String(date.month).padStart(2, "0")}-${String(date.day).padStart(2, "0")}`;
	const onDate = (candidate: PrayerTimes): boolean => format.format(candidate.dhuhr) === wanted;
	if (onDate(times)) {
		return times;
	}
	for (const step of [-1, 1]) {
		const shifted = new Date(Date.UTC(date.year, date.month - 1, date.day + step));
		const neighbour = { year: shifted.getUTCFullYear(), month: shifted.getUTCMonth() + 1, day: shifted.getUTCDate() };
		try {
			const candidate = solarDay(neighbour, location, options);
			if (onDate(candidate)) {
				return candidate;
			}
		} catch {
		}
	}
	return times;
}

function solarDay(date: CivilDate, location: Location, options: Options): PrayerTimes {
	let raw: Float64Array;
	try {
		raw = wasmCompute(
			i16(date.year),
			u8(date.month, "month"),
			u8(date.day, "day"),
			location.latitude,
			location.longitude,
			location.elevation ?? 0,
			options.method ?? "umm_al_qura",
			options.asr ?? "standard",
			options.highLatitudeRule ?? "",
			options.shafaq ?? "",
			ramadanDates(options.ramadan),
		);
	} catch (error) {
		rethrow(error);
	}
	const at = (i: number): Date => new Date(raw[i] ?? Number.NaN);
	const mark = (i: number): Date | null => (Number.isNaN(raw[i] ?? Number.NaN) ? null : at(i));
	return {
		fajr: at(0),
		sunrise: at(1),
		dhuhr: at(2),
		asr: at(3),
		maghrib: at(4),
		isha: at(5),
		midnight: mark(6),
		lastThird: mark(7),
		notes: { fajrHighLatitude: raw[8] === 1, ishaHighLatitude: raw[9] === 1, ramadanIsha: raw[10] === 1, maghribAtSunset: raw[11] === 1 },
	};
}

export function isRamadan(date: CivilDate): boolean {
	try {
		return wasmIsRamadan(i16(date.year), u8(date.month, "month"), u8(date.day, "day"));
	} catch (error) {
		rethrow(error);
	}
}
