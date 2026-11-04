import XCTest
import AppKit
@testable import Hya

final class SettingsAutoDismissTests: XCTestCase {
    func testFiresOnResignKeyOfItsOwnWindow() {
        let window = NSWindow()
        let view = ResignKeyObservingView()
        window.contentView = view

        var fired = false
        view.onResignKey = { fired = true }

        NotificationCenter.default.post(name: NSWindow.didResignKeyNotification, object: window)

        XCTAssertTrue(fired)
    }

    func testDoesNotFireForAnotherWindowsResignKey() {
        let window = NSWindow()
        let otherWindow = NSWindow()
        let view = ResignKeyObservingView()
        window.contentView = view

        var fired = false
        view.onResignKey = { fired = true }

        NotificationCenter.default.post(name: NSWindow.didResignKeyNotification, object: otherWindow)

        XCTAssertFalse(fired)
    }
}
