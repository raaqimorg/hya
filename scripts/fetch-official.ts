// Fetches official timetables into fixtures/official/: the ground truth that tests/official.rs
// checks the presets against. This is the only script that touches the network; the tests read
// the committed JSON and never do. Re-run only to refresh the data, and review the diff.
// Run: bun scripts/fetch-official.ts
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const YEAR = 2026;
const OUT = join(import.meta.dir, "..", "fixtures", "official");

// Every JAKIM zone. Times are computed for each zone's westernmost reference point.
const JAKIM_ZONES = [
  "JHR01", "JHR02", "JHR03", "JHR04",
  "KDH01", "KDH02", "KDH03", "KDH04", "KDH05", "KDH06", "KDH07",
  "KTN01", "KTN02", "MLK01", "NGS01", "NGS02", "NGS03",
  "PHG01", "PHG02", "PHG03", "PHG04", "PHG05", "PHG06", "PLS01", "PNG01",
  "PRK01", "PRK02", "PRK03", "PRK04", "PRK05", "PRK06", "PRK07",
  "SBH01", "SBH02", "SBH03", "SBH04", "SBH05", "SBH06", "SBH07", "SBH08", "SBH09",
  "SGR01", "SGR02", "SGR03",
  "SWK01", "SWK02", "SWK03", "SWK04", "SWK05", "SWK06", "SWK07", "SWK08", "SWK09",
  "TRG01", "TRG02", "TRG03", "TRG04", "WLY01", "WLY02",
];
// Zones whose reference points are published, kept for the whole year; the rest are sampled.
const FULL_YEAR = new Set(["WLY01", "WLY02"]);
const SAMPLED_DAYS = new Set([1, 8, 15, 22]);

const MONTHS: Record<string, number> = {
  Jan: 1, Feb: 2, Mac: 3, Apr: 4, Mei: 5, Jun: 6, Jul: 7, Ogos: 8, Sep: 9, Okt: 10, Nov: 11, Dis: 12,
};

interface JakimDay {
  date: string;
  fajr: string;
  syuruk: string;
  dhuhr: string;
  asr: string;
  maghrib: string;
  isha: string;
}

const hhmm = (t: string) => {
  if (!/^\d\d:\d\d:00$/.test(t)) throw new Error(`unexpected time ${t}`);
  return t.slice(0, 5);
};

async function jakimZone(zone: string): Promise<Record<string, string[]>> {
  const url = `https://www.e-solat.gov.my/index.php?r=esolatApi/takwimsolat&period=year&zone=${zone}`;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${zone}: HTTP ${response.status}`);
  const body = (await response.json()) as { prayerTime: JakimDay[] };
  if (body.prayerTime.length !== 365) throw new Error(`${zone}: ${body.prayerTime.length} days`);
  const days: Record<string, string[]> = {};
  for (const row of body.prayerTime) {
    const [d, mon, y] = row.date.split("-");
    const month = MONTHS[mon ?? ""];
    if (month === undefined || Number(y) !== YEAR) throw new Error(`${zone}: bad date ${row.date}`);
    if (!FULL_YEAR.has(zone) && !SAMPLED_DAYS.has(Number(d))) continue;
    const key = `${y}-${String(month).padStart(2, "0")}-${d}`;
    days[key] = [row.fajr, row.syuruk, row.dhuhr, row.asr, row.maghrib, row.isha].map(hhmm);
  }
  return days;
}

// Jordan's Ministry of Awqaf publishes only from today to 31 December, ten days per page of an
// ASP.NET WebForms grid. Its date-range search fails server-side (HTTP 500), but paging works.
const AWQAF = "https://www.awqaf.gov.jo/ar/Pages/PrayerTime";
const AWQAF_CITIES: Record<string, string> = { "1": "amman", "2": "irbid", "6": "aqaba" };

class WebForm {
  private cookies = new Map<string, string>();
  html = "";

  async send(body?: URLSearchParams): Promise<void> {
    for (let attempt = 0; ; attempt++) {
      const response = await fetch(AWQAF, {
        method: body ? "POST" : "GET",
        body: body ?? null,
        headers: {
          "User-Agent": "hya fixture fetcher (https://github.com/raaqimorg/hya)",
          Cookie: [...this.cookies].map(([k, v]) => `${k}=${v}`).join("; "),
          ...(body ? { "Content-Type": "application/x-www-form-urlencoded", Referer: AWQAF } : {}),
        },
      });
      for (const cookie of response.headers.getSetCookie()) {
        const [pair = ""] = cookie.split(";");
        const eq = pair.indexOf("=");
        this.cookies.set(pair.slice(0, eq), pair.slice(eq + 1));
      }
      if (response.ok) {
        this.html = await response.text();
        return;
      }
      if (attempt === 3) throw new Error(`awqaf: HTTP ${response.status}`);
      await Bun.sleep(10_000 * (attempt + 1));
    }
  }

  /** A postback carrying every field of the current page, as the browser would send it. */
  postback(target: string, argument: string, overrides: Record<string, string> = {}) {
    const form = this.html.slice(this.html.indexOf("<form"));
    const fields = new URLSearchParams();
    for (const tag of form.match(/<input[^>]*>/g) ?? []) {
      const name = /name="([^"]*)"/.exec(tag)?.[1];
      if (name === undefined || /type="submit"/.test(tag)) continue;
      fields.set(unescapeHtml(name), unescapeHtml(/value="([^"]*)"/.exec(tag)?.[1] ?? ""));
    }
    const select = /name="ctl00\$MainContent\$DropCompany".*?<\/select>/s.exec(form)?.[0] ?? "";
    fields.set("ctl00$MainContent$DropCompany", /selected="selected" value="(\d+)"/.exec(select)?.[1] ?? "1");
    for (const [k, v] of Object.entries(overrides)) fields.set(k, v);
    fields.set("__EVENTTARGET", target);
    fields.set("__EVENTARGUMENT", argument);
    return this.send(fields);
  }

  offersPage(page: number): boolean {
    return new RegExp(`gvWebparts(&#39;|'),(&#39;|')Page\\$${page}(&#39;|')`).test(this.html);
  }
}

const unescapeHtml = (s: string) =>
  s.replace(/&amp;/g, "&").replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">");

/** Asr, Maghrib and Isha are printed on a 12-hour clock. */
function awqafRow(cells: string[]): [string, string[]] | undefined {
  const [date, ...times] = cells;
  const match = /^(\d\d)\/(\d\d)\/(\d{4})$/.exec(date ?? "");
  if (!match || times.length !== 6) return undefined;
  const clock = times.map((t, i) => {
    const m = /^(\d\d):(\d\d):00$/.exec(t);
    if (!m) throw new Error(`awqaf: unexpected time ${t}`);
    const hours = Number(m[1]) + (i >= 3 && Number(m[1]) < 12 ? 12 : 0);
    return `${String(hours).padStart(2, "0")}:${m[2]}`;
  });
  return [`${match[3]}-${match[2]}-${match[1]}`, clock];
}

async function awqafCity(id: string): Promise<Record<string, string[]>> {
  const form = new WebForm();
  await form.send();
  if (id !== "1") {
    await form.postback("ctl00$MainContent$DropCompany", "", { "ctl00$MainContent$DropCompany": id });
    // The grid only rebinds to the new city on paging.
    await form.postback("ctl00$MainContent$gvWebparts", "Page$2");
    await form.postback("ctl00$MainContent$gvWebparts", "Page$1");
  }
  const days: Record<string, string[]> = {};
  for (let page = 1; ; page++) {
    const table = form.html.replace(/<script.*?<\/script>/gs, "");
    for (const row of table.match(/<tr[^>]*>.*?<\/tr>/gs) ?? []) {
      const cells = [...row.matchAll(/<td[^>]*>(.*?)<\/td>/gs)].map((c) => c[1]!.replace(/<[^>]+>/g, "").trim());
      const parsed = awqafRow(cells);
      if (parsed) days[parsed[0]] = parsed[1];
    }
    if (!form.offersPage(page + 1)) break;
    await Bun.sleep(2000);
    await form.postback("ctl00$MainContent$gvWebparts", `Page$${page + 1}`);
  }
  return days;
}

// The official Umm al-Qura calculator (ummulqura.org.sa) is backed by this KACST API, which
// computes for exact coordinates: no reference point to guess.
const UMM_AL_QURA_CITIES: Record<string, [number, number]> = {
  makkah: [21.4225, 39.8262],
  riyadh: [24.7136, 46.6753],
  tabuk: [28.3835, 36.5662],
};

const USER_AGENT = { "User-Agent": "hya fixture fetcher (https://github.com/raaqimorg/hya)" };
const hhmmOnly = (t: string) => {
  if (!/^\d\d:\d\d$/.test(t)) throw new Error(`unexpected time ${t}`);
  return t;
};

async function ummAlQuraCity([lat, lng]: [number, number]): Promise<Record<string, string[]>> {
  const url = `https://umqserv.kacst.gov.sa/api/v1/Prayer/GetPrayerByYear?lang=en&format=24&yg=${YEAR}&lat=${lat}&lon=${lng}&zone=3`;
  const response = await fetch(url, { headers: USER_AGENT });
  if (!response.ok) throw new Error(`umm al-qura: HTTP ${response.status}`);
  const body = (await response.json()) as { date: string; prayerTimes: Partial<Record<"fajr" | "sunrise" | "dhuhr" | "asr" | "maghrib" | "isha", string>> }[];
  if (body.length !== 365) throw new Error(`umm al-qura: ${body.length} days`);
  const days: Record<string, string[]> = {};
  for (const { date, prayerTimes: p } of body) {
    days[date.slice(0, 10)] = [p.fajr, p.sunrise, p.dhuhr, p.asr, p.maghrib, p.isha].map((t) => hhmmOnly(t ?? ""));
  }
  return days;
}

// MUIS publishes one timetable for the whole island on data.gov.sg (Singapore Open Data Licence).
const MUIS_DATASET = "d_d441e7242e78efc566024dd5b0d9829c";

async function muis(): Promise<Record<string, string[]>> {
  const url = `https://data.gov.sg/api/action/datastore_search?resource_id=${MUIS_DATASET}&limit=400`;
  const response = await fetch(url, { headers: USER_AGENT });
  if (!response.ok) throw new Error(`muis: HTTP ${response.status}`);
  const body = (await response.json()) as { result: { records: Partial<Record<"Date" | "Subuh" | "Syuruk" | "Zohor" | "Asar" | "Maghrib" | "Isyak", string>>[] } };
  const records = body.result.records;
  if (records.length !== 365) throw new Error(`muis: ${records.length} days`);
  const days: Record<string, string[]> = {};
  for (const r of records) {
    days[r.Date ?? ""] = [r.Subuh, r.Syuruk, r.Zohor, r.Asar, r.Maghrib, r.Isyak].map((t) => hhmmOnly(t ?? ""));
  }
  return days;
}

const ummAlQura: Record<string, { latitude: number; longitude: number; days: Record<string, string[]> }> = {};
for (const [city, point] of Object.entries(UMM_AL_QURA_CITIES)) {
  ummAlQura[city] = { latitude: point[0], longitude: point[1], days: await ummAlQuraCity(point) };
  await Bun.sleep(2000);
}
const singapore = await muis();

const zones: Record<string, Record<string, string[]>> = {};
for (const zone of JAKIM_ZONES) {
  zones[zone] = await jakimZone(zone);
  await Bun.sleep(1000); // be polite to a public government service
}

const jordan: Record<string, Record<string, string[]>> = {};
for (const [id, city] of Object.entries(AWQAF_CITIES)) {
  jordan[city] = await awqafCity(id);
  if (Object.keys(jordan[city]).length < 30) throw new Error(`awqaf ${city}: too few days`);
}

mkdirSync(OUT, { recursive: true });
writeFileSync(
  join(OUT, `umm-al-qura-${YEAR}.json`),
  `${JSON.stringify(
    {
      source: "Official Umm al-Qura calendar, https://www.ummulqura.org.sa/ar/prayer-times (KACST API umqserv.kacst.gov.sa, GetPrayerByYear)",
      retrieved: new Date().toISOString().slice(0, 10),
      note: "Computed by the authority for each city's exact coordinates. Local time UTC+3. Each row: fajr, sunrise, dhuhr, asr, maghrib, isha.",
      cities: ummAlQura,
    },
    null,
    1,
  )}\n`,
);
writeFileSync(
  join(OUT, `muis-${YEAR}.json`),
  `${JSON.stringify(
    {
      source: `Majlis Ugama Islam Singapura, Muslim Prayer Timetable ${YEAR}, data.gov.sg dataset ${MUIS_DATASET}`,
      licence: "Singapore Open Data Licence, https://data.gov.sg/open-data-licence",
      retrieved: new Date().toISOString().slice(0, 10),
      note: "One timetable for all of Singapore; MUIS does not publish its reference point. Local time UTC+8. Each row: fajr (subuh), sunrise (syuruk), dhuhr, asr, maghrib, isha.",
      days: singapore,
    },
    null,
    1,
  )}\n`,
);
writeFileSync(
  join(OUT, `awqaf-jordan-${YEAR}.json`),
  `${JSON.stringify(
    {
      source: "Ministry of Awqaf, Islamic Affairs and Holy Places (Jordan), https://www.awqaf.gov.jo/ar/Pages/PrayerTime",
      retrieved: new Date().toISOString().slice(0, 10),
      note: "Official Jordanian timetables, local time UTC+3, 24-hour. The ministry publishes only from the day of retrieval to 31 December. Each row: fajr, sunrise, dhuhr, asr, maghrib, isha.",
      cities: jordan,
    },
    null,
    1,
  )}\n`,
);
writeFileSync(
  join(OUT, `jakim-${YEAR}.json`),
  `${JSON.stringify(
    {
      source: "JAKIM e-solat, https://www.e-solat.gov.my (esolatApi/takwimsolat, period=year)",
      retrieved: new Date().toISOString().slice(0, 10),
      note: "Official Malaysian timetables, local time UTC+8. Each row: fajr, syuruk, dhuhr, asr, maghrib, isha.",
      referencePoints: {
        source:
          "Jabatan Mufti Wilayah Persekutuan, Bayan al-Falak siri ke-17, citing Almanak Falak Syarie Malaysia 2021 (JUPEM): https://muftiwp.gov.my/ms/artikel/bayan-al-falak/5381-bayan-al-falak-siri-ke-17-kaedah-penentuan-waktu-solat",
        WLY01: { prayers: "Kg. Gedangsa 3°44′N 101°23′E", syuruk: "Keramat, KL 3°11′24″N 101°45′30″E" },
        WLY02: { prayers: "Pulau Kuraman 5°14′53″N 115°08′23″E", syuruk: "Pulau Daat 5°17′23″N 115°20′34″E" },
      },
      zones,
    },
    null,
    1,
  )}\n`,
);
console.log(`wrote ${Object.keys(zones).length} zones`);
