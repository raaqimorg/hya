import XCTest
@testable import Hya

final class HyaProcessTests: XCTestCase {
    func testMissingExecutableReturnsNotFound() {
        let result = runHya(executableURL: URL(fileURLWithPath: "/nonexistent/hya"), coordinates: Coordinates(latitude: 0, longitude: 0))
        XCTAssertEqual(result, .failure(.notFound))
    }

    func testNonZeroExitIsSurfaced() {
        let result = runHya(executableURL: URL(fileURLWithPath: "/usr/bin/false"), coordinates: Coordinates(latitude: 0, longitude: 0))
        XCTAssertEqual(result, .failure(.nonZeroExit(1)))
    }

    func testInvalidOutputIsSurfaced() {
        let result = runHya(executableURL: URL(fileURLWithPath: "/bin/echo"), coordinates: Coordinates(latitude: 0, longitude: 0))
        XCTAssertEqual(result, .failure(.invalidOutput))
    }
}
