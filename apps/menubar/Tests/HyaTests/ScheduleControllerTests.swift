import XCTest
import AppKit
@testable import Hya

final class ScheduleControllerTests: XCTestCase {
    /// Regression test for the post-sleep bug: the menu bar showed bare "hya" (no timing)
    /// after the Mac woke from a long sleep. The wake handler used to only recompute display
    /// text from the snapshot already in memory, which is stale once the Mac has slept past
    /// the day it was fetched for; every prayer in it (and `tomorrowFajr`) is now in the
    /// past, so `statusText` falls through to its "hya" catch-all. Waking must refetch.
    func testWakeNotificationRefetchesInsteadOfReusingStaleSnapshot() throws {
        let markerURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("hya-wake-fixture-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: markerURL) }

        let freshFajrAt = Int64(Date().addingTimeInterval(600).timeIntervalSince1970)
        let scriptURL = try Self.makeSequencedFixtureScript(markerURL: markerURL, freshFajrAt: freshFajrAt)
        defer { try? FileManager.default.removeItem(at: scriptURL) }

        let controller = ScheduleController(executableURL: scriptURL)
        controller.start(coordinates: Coordinates(latitude: 21.4225, longitude: 39.8262))
        defer { controller.stop() }

        XCTAssertEqual(controller.displayText, "hya", "sanity check: first fetch is the stale, all-past snapshot")

        NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.didWakeNotification, object: nil)

        XCTAssertTrue(
            controller.displayText.hasPrefix("Fjr"),
            "waking should refetch rather than recompute from the stale pre-sleep snapshot; got \(controller.displayText)"
        )
    }

    /// A fixture script that returns an all-past snapshot (today's prayers at epoch 0, no
    /// `tomorrow_fajr`) on its first invocation, and a snapshot with Fajr `freshFajrAt` seconds
    /// away on every later invocation — so a test can tell a refetch happened from one that didn't.
    private static func makeSequencedFixtureScript(markerURL: URL, freshFajrAt: Int64) throws -> URL {
        let staleJSON = """
        {"generated_at":0,"today":{"fajr":{"at":0,"ends":null},"sunrise":{"at":0,"ends":null},"dhuhr":{"at":0,"ends":null},"asr":{"at":0,"ends":null},"maghrib":{"at":0,"ends":null},"isha":{"at":0,"ends":null}},"yesterday_isha":null,"tomorrow_fajr":null}
        """
        let freshJSON = """
        {"generated_at":0,"today":{"fajr":{"at":\(freshFajrAt),"ends":null},"sunrise":{"at":\(freshFajrAt),"ends":null},"dhuhr":{"at":\(freshFajrAt),"ends":null},"asr":{"at":\(freshFajrAt),"ends":null},"maghrib":{"at":\(freshFajrAt),"ends":null},"isha":{"at":\(freshFajrAt),"ends":null}},"yesterday_isha":null,"tomorrow_fajr":null}
        """
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("hya-fixture-\(UUID().uuidString)")
        let script = """
        #!/bin/sh
        if [ -f "\(markerURL.path)" ]; then
          cat <<'EOF'
        \(freshJSON)
        EOF
        else
          touch "\(markerURL.path)"
          cat <<'EOF'
        \(staleJSON)
        EOF
        fi
        """
        try script.write(to: url, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: url.path)
        return url
    }

    func testNextLocalMidnightIsStrictlyAfterNow() {
        let calendar = Calendar(identifier: .gregorian)
        let now = Date(timeIntervalSince1970: 1_700_000_000)
        let midnight = nextLocalMidnight(after: now, calendar: calendar)
        XCTAssertGreaterThan(midnight, now)
        XCTAssertEqual(calendar.component(.hour, from: midnight), 0)
        XCTAssertEqual(calendar.component(.minute, from: midnight), 0)
    }

    func testNextLocalMidnightAtExactlyMidnightReturnsTheFollowingOne() {
        let calendar = Calendar(identifier: .gregorian)
        let midnight = calendar.startOfDay(for: Date(timeIntervalSince1970: 1_700_000_000))
        let next = nextLocalMidnight(after: midnight, calendar: calendar)
        XCTAssertEqual(next.timeIntervalSince(midnight), 86400, accuracy: 1)
    }
}
