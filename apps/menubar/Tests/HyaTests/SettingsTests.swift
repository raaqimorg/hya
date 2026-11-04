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

    func testInitialTextIsEmptyWithNoSavedCoordinates() {
        let initial = SettingsView.initialText(for: nil)
        XCTAssertEqual(initial.latitude, "")
        XCTAssertEqual(initial.longitude, "")
    }

    func testInitialTextReflectsSavedCoordinates() {
        let initial = SettingsView.initialText(for: Coordinates(latitude: 24.7136, longitude: 46.6753))
        XCTAssertEqual(initial.latitude, "24.7136")
        XCTAssertEqual(initial.longitude, "46.6753")
    }

    func testMuteBeforePrayerIsOffByDefault() {
        let defaults = UserDefaults(suiteName: #function)!
        defaults.removePersistentDomain(forName: #function)
        let store = SettingsStore(defaults: defaults)
        XCTAssertFalse(store.muteBeforePrayerEnabled)
    }

    func testMuteBeforePrayerSetThenReadRoundTrips() {
        let defaults = UserDefaults(suiteName: #function)!
        defaults.removePersistentDomain(forName: #function)
        let store = SettingsStore(defaults: defaults)
        store.setMuteBeforePrayerEnabled(true)
        XCTAssertTrue(store.muteBeforePrayerEnabled)
        XCTAssertTrue(SettingsStore(defaults: defaults).muteBeforePrayerEnabled)
    }
}
