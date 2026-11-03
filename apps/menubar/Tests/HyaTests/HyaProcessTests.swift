import XCTest
@testable import Hya

final class HyaProcessTests: XCTestCase {
    func testMissingExecutableReturnsNotFound() {
        let result = runHya(executableURL: URL(fileURLWithPath: "/nonexistent/hya"), coordinates: Coordinates(latitude: 0, longitude: 0))
        XCTAssertEqual(result, .failure(.notFound))
    }

    func testNonZeroExitIsSurfaced() {
        // /usr/bin/false always exits 1; stands in for a bundled binary that fails.
        let result = runHya(executableURL: URL(fileURLWithPath: "/usr/bin/false"), coordinates: Coordinates(latitude: 0, longitude: 0))
        XCTAssertEqual(result, .failure(.nonZeroExit(1)))
    }

    func testInvalidOutputIsSurfaced() {
        // /bin/echo "" always exits 0 with output that is not a PrayerSnapshot.
        let result = runHya(executableURL: URL(fileURLWithPath: "/bin/echo"), coordinates: Coordinates(latitude: 0, longitude: 0))
        XCTAssertEqual(result, .failure(.invalidOutput))
    }
}
