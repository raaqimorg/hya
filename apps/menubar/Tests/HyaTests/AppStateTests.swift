import XCTest
import Combine
import AppKit
@testable import Hya

final class AppStateTests: XCTestCase {
    func testWakeNotificationRefreshedDisplayTextNotifiesAppStateObservers() throws {
        SettingsStore.shared.save(Coordinates(latitude: 21.4225, longitude: 39.8262))
        let scriptURL = try Self.makeFixtureScript()
        defer { try? FileManager.default.removeItem(at: scriptURL) }

        let appState = AppState(executableURL: scriptURL)
        var changeCount = 0
        let cancellable = appState.objectWillChange.sink { _ in changeCount += 1 }
        defer { cancellable.cancel() }

        NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.didWakeNotification, object: nil)

        XCTAssertGreaterThan(changeCount, 0, "AppState should notify its own observers when the schedule controller refreshes after wake")
    }

    func testApplyLocationPersistsAndKeepsModeReady() throws {
        SettingsStore.shared.save(Coordinates(latitude: 0, longitude: 0))
        let scriptURL = try Self.makeFixtureScript()
        defer { try? FileManager.default.removeItem(at: scriptURL) }

        let appState = AppState(executableURL: scriptURL)
        let coordinates = Coordinates(latitude: 21.4225, longitude: 39.8262)
        appState.applyLocation(coordinates)

        XCTAssertEqual(appState.mode, .ready)
        XCTAssertEqual(SettingsStore.shared.coordinates, coordinates)
    }

    func testResetLocationClearsSettingsAndReturnsToNeedsLocation() throws {
        SettingsStore.shared.save(Coordinates(latitude: 21.4225, longitude: 39.8262))
        let scriptURL = try Self.makeFixtureScript()
        defer { try? FileManager.default.removeItem(at: scriptURL) }

        let appState = AppState(executableURL: scriptURL)
        XCTAssertEqual(appState.mode, .ready)

        appState.resetLocation()

        XCTAssertEqual(appState.mode, .needsLocation)
        XCTAssertNil(SettingsStore.shared.coordinates)
    }

    private static func makeFixtureScript() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("hya-fixture-\(UUID().uuidString)")
        let json = """
        {"generated_at":0,"today":{"fajr":{"at":0,"ends":null},"sunrise":{"at":0,"ends":null},"dhuhr":{"at":0,"ends":null},"asr":{"at":0,"ends":null},"maghrib":{"at":0,"ends":null},"isha":{"at":0,"ends":null}},"yesterday_isha":null,"tomorrow_fajr":null}
        """
        let script = "#!/bin/sh\ncat <<'EOF'\n\(json)\nEOF\n"
        try script.write(to: url, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: url.path)
        return url
    }
}
