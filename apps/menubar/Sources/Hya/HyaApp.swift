import SwiftUI

@main
struct HyaApp: App {
    init() {
        NSApplication.shared.setActivationPolicy(.accessory)
    }

    var body: some Scene {
        MenuBarExtra("hya") {
            Text("hya")
        }
    }
}
