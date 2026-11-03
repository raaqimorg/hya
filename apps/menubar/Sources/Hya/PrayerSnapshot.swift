import Foundation

struct Window: Decodable {
    let at: Int64
    let ends: Int64?
}

struct Today: Decodable {
    let fajr: Window
    let sunrise: Window
    let dhuhr: Window
    let asr: Window
    let maghrib: Window
    let isha: Window
}

struct PrayerSnapshot: Decodable {
    let generatedAt: Int64
    let today: Today
    let yesterdayIsha: Window?
    let tomorrowFajr: Int64?

    enum CodingKeys: String, CodingKey {
        case generatedAt = "generated_at"
        case today
        case yesterdayIsha = "yesterday_isha"
        case tomorrowFajr = "tomorrow_fajr"
    }
}

func formatDuration(seconds: Int64) -> String {
    if seconds < 3600 {
        return "\(seconds / 60)m"
    }
    let hours = seconds / 3600
    let minutes = (seconds % 3600) / 60
    return "\(hours)h:\(String(format: "%02d", minutes))m"
}

func statusText(for snapshot: PrayerSnapshot, at now: Date) -> String {
    let nowSeconds = Int64(now.timeIntervalSince1970)
    let today = snapshot.today

    var active: [(name: String, at: Int64, ends: Int64?)] = [
        ("Fajr", today.fajr.at, today.fajr.ends),
        ("Sunrise", today.sunrise.at, today.sunrise.ends),
        ("Dhuhr", today.dhuhr.at, today.dhuhr.ends),
        ("Asr", today.asr.at, today.asr.ends),
        ("Maghrib", today.maghrib.at, today.maghrib.ends),
        ("Isha", today.isha.at, today.isha.ends),
    ]
    if let yesterdayIsha = snapshot.yesterdayIsha {
        active.append(("Isha", yesterdayIsha.at, yesterdayIsha.ends))
    }

    if let current = active.first(where: { entry in
        guard let ends = entry.ends else { return false }
        return (entry.at...ends).contains(nowSeconds)
    }), let ends = current.ends {
        return "\(current.name) (-\(formatDuration(seconds: ends - nowSeconds)))"
    }

    var upcoming: [(name: String, at: Int64)] = [
        ("Fajr", today.fajr.at),
        ("Dhuhr", today.dhuhr.at),
        ("Asr", today.asr.at),
        ("Maghrib", today.maghrib.at),
        ("Isha", today.isha.at),
    ]
    if let tomorrowFajr = snapshot.tomorrowFajr {
        upcoming.append(("Fajr", tomorrowFajr))
    }
    if let next = upcoming.filter({ $0.at > nowSeconds }).min(by: { $0.at < $1.at }) {
        return "\(next.name) (+\(formatDuration(seconds: next.at - nowSeconds)))"
    }

    return "hya"
}
