import XCTest
@testable import Hya

final class ScheduleControllerTests: XCTestCase {
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
