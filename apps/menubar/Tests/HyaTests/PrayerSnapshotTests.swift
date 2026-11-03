import XCTest
@testable import Hya

final class PrayerSnapshotTests: XCTestCase {
    func testFormatDurationUnderAnHour() {
        XCTAssertEqual(formatDuration(seconds: 59 * 60), "59m")
    }

    func testFormatDurationAtExactlyAnHour() {
        XCTAssertEqual(formatDuration(seconds: 60 * 60), "1h:00m")
    }

    func testFormatDurationPadsMinutes() {
        XCTAssertEqual(formatDuration(seconds: 61 * 60 + 5), "1h:01m")
    }

    func testFormatDurationAtZero() {
        XCTAssertEqual(formatDuration(seconds: 0), "0m")
    }

    func testStatusTextInsideTodaysWindowShowsMinus() {
        let snapshot = fixture(asr: Window(at: 1000, ends: 2000)) // 1000s=16m40s window
        XCTAssertEqual(statusText(for: snapshot, at: Date(timeIntervalSince1970: 1000 + 12 * 60)), "Asr -4m")
    }

    func testStatusTextWaitingForNextShowsPlus() {
        let snapshot = fixture(maghrib: Window(at: 5000, ends: 5600))
        XCTAssertEqual(statusText(for: snapshot, at: Date(timeIntervalSince1970: 5000 - 38 * 60)), "Mag +38m")
    }

    func testStatusTextJustAfterMidnightUsesYesterdaysIsha() {
        let snapshot = fixture(yesterdayIsha: Window(at: -300, ends: 1500)) // still open 1200s after midnight
        XCTAssertEqual(statusText(for: snapshot, at: Date(timeIntervalSince1970: 300)), "Ish -20m")
    }
}

private let neutralWindow = Window(at: 0, ends: nil)

private func fixture(
    fajr: Window = neutralWindow,
    sunrise: Window = neutralWindow,
    dhuhr: Window = neutralWindow,
    asr: Window = neutralWindow,
    maghrib: Window = neutralWindow,
    isha: Window = neutralWindow,
    yesterdayIsha: Window? = nil,
    tomorrowFajr: Int64? = nil
) -> PrayerSnapshot {
    PrayerSnapshot(
        generatedAt: 0,
        today: Today(fajr: fajr, sunrise: sunrise, dhuhr: dhuhr, asr: asr, maghrib: maghrib, isha: isha),
        yesterdayIsha: yesterdayIsha,
        tomorrowFajr: tomorrowFajr
    )
}
