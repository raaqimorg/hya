import SwiftUI

@main
struct HyaApp: App {
    @StateObject private var appState = AppState(executableURL: AppState.bundledExecutableURL)

    init() {
        NSApplication.shared.setActivationPolicy(.accessory)
    }

    var body: some Scene {
        MenuBarExtra(label) {
            content
        }
    }

    private var label: String {
        switch appState.mode {
        case .needsLocation:
            return "hya: Set location"
        case .ready:
            return appState.scheduleController.displayText
        case .error:
            return "hya: No times"
        }
    }

    @ViewBuilder
    private var content: some View {
        switch appState.mode {
        case .needsLocation:
            SettingsView(
                onSave: { appState.settingsSaved() },
                onCancel: SettingsStore.shared.coordinates != nil ? { appState.mode = .ready } : nil
            )
        case .ready:
            ScheduleListView(
                scheduleController: appState.scheduleController,
                onSettings: { appState.mode = .needsLocation }
            )
        case .error(let error):
            VStack(alignment: .leading, spacing: 4) {
                Text(description(for: error))
                Divider()
                HStack {
                    Button("Settings…") { appState.mode = .needsLocation }
                    Spacer()
                    Button("Quit") { NSApplication.shared.terminate(nil) }
                }
            }
            .padding()
        }
    }

    private func description(for error: HyaProcessError) -> String {
        switch error {
        case .notFound:
            return "The hya binary could not be found."
        case .nonZeroExit(let code):
            return "hya exited with status \(code)."
        case .invalidOutput:
            return "hya produced output this app could not read."
        }
    }
}
