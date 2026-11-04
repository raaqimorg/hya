import Foundation

struct Window: Decodable, Equatable {
    let at: Int64
    let ends: Int64?
}

struct Today: Decodable, Equatable {
    let fajr: Window
    let sunrise: Window
    let dhuhr: Window
    let asr: Window
    let maghrib: Window
    let isha: Window
}

struct PrayerSnapshot: Decodable, Equatable {
    let generatedAt: Int64
    let today: Today
    let midnight: Int64?
    let lastThird: Int64?
    let yesterdayIsha: Window?
    let tomorrowFajr: Int64?

    enum CodingKeys: String, CodingKey {
        case generatedAt = "generated_at"
        case today
        case midnight
        case lastThird = "last_third"
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

/// Whether `now` falls inside `window`'s own `at...ends` span. Always `false` for a window
/// with no `ends` (e.g. Sunrise).
func isActive(_ window: Window, at now: Date) -> Bool {
    guard let ends = window.ends else { return false }
    let nowSeconds = Int64(now.timeIntervalSince1970)
    return (window.at...ends).contains(nowSeconds)
}

func statusText(for snapshot: PrayerSnapshot, at now: Date) -> String {
    let nowSeconds = Int64(now.timeIntervalSince1970)
    let today = snapshot.today

    var active: [(name: String, window: Window)] = [
        ("Fjr", today.fajr),
        ("Sun", today.sunrise),
        ("Dhu", today.dhuhr),
        ("Asr", today.asr),
        ("Mag", today.maghrib),
        ("Ish", today.isha),
    ]
    if let yesterdayIsha = snapshot.yesterdayIsha {
        active.append(("Ish", yesterdayIsha))
    }

    if let current = active.first(where: { isActive($0.window, at: now) }) {
        return "\(current.name) -\(formatDuration(seconds: nowSeconds - current.window.at))"
    }

    var upcoming: [(name: String, at: Int64)] = [
        ("Fjr", today.fajr.at),
        ("Dhu", today.dhuhr.at),
        ("Asr", today.asr.at),
        ("Mag", today.maghrib.at),
        ("Ish", today.isha.at),
    ]
    if let tomorrowFajr = snapshot.tomorrowFajr {
        upcoming.append(("Fjr", tomorrowFajr))
    }
    if let next = upcoming.filter({ $0.at > nowSeconds }).min(by: { $0.at < $1.at }) {
        return "\(next.name) +\(formatDuration(seconds: next.at - nowSeconds))"
    }

    return "hya"
}

/// Today's prayer whose 5-minute pre-window `now` currently falls in, i.e. `[at - 5m, at)`, unless
/// `lastTriggered` is already that prayer's `at` (it already fired for this window). Sunrise has no
/// adhan, so it is never a candidate.
func prayerToMute(for snapshot: PrayerSnapshot, at now: Date, lastTriggered: Int64?) -> (name: String, at: Int64)? {
    let nowSeconds = Int64(now.timeIntervalSince1970)
    let today = snapshot.today
    let leadSeconds: Int64 = 5 * 60

    let prayers: [(name: String, at: Int64)] = [
        ("Fajr", today.fajr.at),
        ("Dhuhr", today.dhuhr.at),
        ("Asr", today.asr.at),
        ("Maghrib", today.maghrib.at),
        ("Isha", today.isha.at),
    ]
    return prayers.first { prayer in
        prayer.at != lastTriggered && (prayer.at - leadSeconds..<prayer.at).contains(nowSeconds)
    }
}
