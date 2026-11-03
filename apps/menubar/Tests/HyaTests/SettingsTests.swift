import XCTest
@testable import Hya

final class SettingsTests: XCTestCase {
    func testValidatedAcceptsBoundaryValues() {
        XCTAssertEqual(try? Coordinates.validated(latitude: "90", longitude: "-180").get(), Coordinates(latitude: 90, longitude: -180))
    }

    func testValidatedRejectsOutOfRangeLatitude() {
        XCTAssertThrowsError(try Coordinates.validated(latitude: "90.1", longitude: "0").get())
    }

    func testValidatedRejectsNonNumericInput() {
        XCTAssertThrowsError(try Coordinates.validated(latitude: "abc", longitude: "0").get())
    }

    func testSaveThenLoadRoundTrips() {
        let defaults = UserDefaults(suiteName: #function)!
        defaults.removePersistentDomain(forName: #function)
        let store = SettingsStore(defaults: defaults)
        XCTAssertNil(store.coordinates)
        store.save(Coordinates(latitude: 24.7136, longitude: 46.6753))
        XCTAssertEqual(store.coordinates, Coordinates(latitude: 24.7136, longitude: 46.6753))
    }
}
